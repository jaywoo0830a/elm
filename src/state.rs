//! State — arena slots. `State<T>` is a `Copy` index handle.
//!
//! 세 종류의 저장소가 있다:
//! - **지역 슬롯**: 컴포넌트 매개변수·라이프사이클 슬롯 (숫자 인덱스)
//! - **keyed 슬롯**: 인스턴스 경로(`<Row key={…}>`)별로 안정적인 슬롯 (사양서 9.5)
//! - **store 슬롯**: `#[store]` 전역 상태 — 이름 기반 (사양서 4.2)

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;

/// A deferred effect continuation: runs with the arena once its future resolves.
pub type PendingEffect = Box<dyn FnOnce(&mut Arena)>;

/// A live stream task: pumps one value; returns false when exhausted.
pub type StreamTask = Box<dyn FnMut(&mut Arena) -> bool>;

/// Keyboard handler registered by `on_key` (사양서 5.3).
pub type KeyHandler = Rc<dyn Fn(&mut Arena)>;

/// `on_navigate` 핸들러 — 새 경로/라우트 값을 타입 소거된 채로 받는다 (사양서 5.3).
///
/// 핸들러는 `::elm_magic::nav_take::<T>(값)`으로 꺼내 쓴다 (타입은 본문에서 추론).
pub type NavHandler = Rc<dyn Fn(&mut Arena, &dyn Any)>;

/// A queued effect with its due time (ms on the simulated clock).
struct Pending {
    due: u64,
    run: PendingEffect,
}

/// 한 프레임이 끝난 뒤 전달할 것들 (unmount / 구독 이벤트).
#[derive(Default)]
pub struct FrameTail {
    unmounts: Vec<KeyHandler>,
    events: Vec<KeyHandler>,
    net: Vec<KeyHandler>,
    navs: Vec<(Rc<dyn Any>, NavHandler)>,
}

impl FrameTail {
    pub fn is_empty(&self) -> bool {
        self.unmounts.is_empty()
            && self.events.is_empty()
            && self.net.is_empty()
            && self.navs.is_empty()
    }

    /// 전달: unmount → on_event → on_net_change → on_navigate 순.
    pub fn run(self, arena: &mut Arena) {
        for h in self.unmounts {
            h(arena);
        }
        for h in self.events {
            h(arena);
        }
        for h in self.net {
            h(arena);
        }
        for (value, h) in self.navs {
            h(arena, &*value);
        }
    }
}

/// Slot storage. All component state lives here; nothing else is mutated.
#[derive(Default)]
pub struct Arena {
    slots: Vec<Option<Box<dyn Any>>>,
    /// `#[store]` 전역 상태 — `"App.user"` 같은 이름 키 (사양서 4.2).
    stores: HashMap<&'static str, Box<dyn Any>>,
    /// store 필드별 버전 카운터 (사양서 4.2 — 변경 추적).
    store_versions: HashMap<&'static str, u64>,
    /// keyed 슬롯: `"인스턴스경로#idx"` → 슬롯 인덱스 (사양서 9.5).
    key_index: HashMap<String, usize>,
    key_next: usize,
    /// Effects spawned by `<-` — run by `flush()` (사양서 12: Cmd는 flush 전까지 실행 안 됨).
    /// `<- f() after 300ms`는 `due`가 미래라 `advance(ms)`가 지나야 실행된다.
    pending: Vec<Pending>,
    /// Simulated clock in ms (사양서 8.2). `advance`가 전진시킨다.
    now: u64,
    /// Live streams registered by `->` — pumped by `pump()` (사양서 5.4).
    streams: Vec<StreamTask>,
    /// `bus.emit(...)` 큐 — 다음 프레임에서 배달된다 (사양서 5.3).
    emitted: Vec<String>,
    /// `net.is_online()` (사양서 5.3).
    online: bool,
    online_changed: bool,
    /// `navigate(...)` 큐 — 다음 프레임에서 `on_navigate`가 받는다.
    navigations: Vec<Rc<dyn Any>>,
}

impl Arena {
    pub fn new() -> Self {
        Arena { online: true, ..Self::default() }
    }

    /// Simulated clock (ms).
    pub fn now(&self) -> u64 {
        self.now
    }

    /// Set the simulated clock (used by `TestApp::advance`).
    pub fn set_now(&mut self, t: u64) {
        self.now = t;
    }

    fn ensure_len(&mut self, idx: usize) {
        if self.slots.len() <= idx {
            self.slots.resize_with(idx + 1, || None);
        }
    }

    pub fn slot<T: 'static>(&mut self, idx: usize, init: impl FnOnce() -> T) -> State<T> {
        self.ensure_len(idx);
        if self.slots[idx].is_none() {
            self.slots[idx] = Some(Box::new(init()));
        }
        State { key: SlotKey::Index(idx), _pd: PhantomData }
    }

    pub fn get<T: 'static>(&self, idx: usize) -> &T {
        self.slots[idx]
            .as_ref()
            .expect("slot not initialized")
            .downcast_ref::<T>()
            .expect("slot type mismatch")
    }

    pub fn set<T: 'static>(&mut self, idx: usize, value: T) {
        self.slots[idx] = Some(Box::new(value));
    }

    pub fn mutate<T: 'static, F: FnOnce(&mut T)>(&mut self, idx: usize, f: F) {
        let v = self.get_mut::<T>(idx);
        f(v);
    }

    fn get_mut<T: 'static>(&mut self, idx: usize) -> &mut T {
        self.slots[idx]
            .as_mut()
            .expect("slot not initialized")
            .downcast_mut::<T>()
            .expect("slot type mismatch")
    }

    // ── store: 전역 상태 (사양서 4.2) ────────────────────────

    /// 전역 상태 슬롯. 이름이 키라서 컴포넌트 사이에 공유된다.
    pub fn store<T: 'static>(&mut self, key: &'static str, init: impl FnOnce() -> T) -> State<T> {
        if !self.stores.contains_key(key) {
            self.stores.insert(key, Box::new(init()));
            self.store_versions.insert(key, 0);
        }
        State { key: SlotKey::Store(key), _pd: PhantomData }
    }

    pub fn store_get<T: 'static>(&self, key: &'static str) -> &T {
        self.stores
            .get(key)
            .and_then(|v| v.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("store `{}` not initialized (type mismatch?)", key))
    }

    pub fn store_set<T: 'static>(&mut self, key: &'static str, value: T) {
        self.stores.insert(key, Box::new(value));
        *self.store_versions.entry(key).or_insert(0) += 1;
    }

    pub fn store_mutate<T: 'static, F: FnOnce(&mut T)>(&mut self, key: &'static str, f: F) {
        match self.stores.get_mut(key).and_then(|v| v.downcast_mut::<T>()) {
            Some(v) => {
                f(v);
                *self.store_versions.entry(key).or_insert(0) += 1;
            }
            None => panic!("store `{}` not initialized (type mismatch?)", key),
        }
    }

    /// store 필드의 버전 카운터 — 변경 추적 (사양서 4.2).
    pub fn store_version(&self, key: &'static str) -> u64 {
        *self.store_versions.get(key).unwrap_or(&0)
    }

    // ── keyed 슬롯 (사양서 9.5) ─────────────────────────────

    /// 인스턴스 경로별 슬롯: 같은 키는 같은 슬롯(상태 보존),
    /// 사라진 키는 `drop_instance`가 초기화한다.
    pub fn keyed_slot<T: 'static>(&mut self, key: &str, init: impl FnOnce() -> T) -> State<T> {
        let idx = match self.key_index.get(key) {
            Some(i) => *i,
            None => {
                let i = self.key_next;
                self.key_next += 1;
                self.key_index.insert(key.to_string(), i);
                i
            }
        };
        self.ensure_len(idx);
        if self.slots[idx].is_none() {
            self.slots[idx] = Some(Box::new(init()));
        }
        State { key: SlotKey::Index(idx), _pd: PhantomData }
    }

    /// 인스턴스 경로가 사라졌을 때: 그 경로(와 하위)의 슬롯을 버린다 → 상태 초기화.
    pub fn drop_instance(&mut self, path: &str) {
        let prefix = format!("{}/", path);
        let dead: Vec<String> = self
            .key_index
            .keys()
            .filter(|k| {
                let p = k.split('#').next().unwrap_or("");
                p == path || p.starts_with(&prefix)
            })
            .cloned()
            .collect();
        for k in dead {
            if let Some(idx) = self.key_index.remove(&k) {
                self.slots[idx] = None;
            }
        }
    }

    /// 살아있는 keyed 슬롯 수 (테스트/디버깅).
    pub fn keyed_slot_count(&self) -> usize {
        self.key_index.len()
    }

    // ── 이벤트 버스 / 네트워크 / 라우팅 (사양서 5.3) ────────

    /// `bus.emit(Name)` — 다음 프레임에서 `on_event` 핸들러가 받는다.
    pub fn emit(&mut self, name: &str) {
        let n = name.replace(' ', "");
        if !self.emitted.contains(&n) {
            self.emitted.push(n);
        }
    }

    pub fn take_emitted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.emitted)
    }

    /// `net.is_online()`.
    pub fn online(&self) -> bool {
        self.online
    }

    /// 온라인/오프라인 전환 — `on_net_change` 핸들러가 다음 프레임에 실행된다.
    pub fn set_online(&mut self, online: bool) {
        if self.online != online {
            self.online = online;
            self.online_changed = true;
        }
    }

    pub fn take_online_changed(&mut self) -> bool {
        std::mem::take(&mut self.online_changed)
    }

    /// `navigate(값)` — 다음 프레임에서 `on_navigate` 핸들러가 받는다.
    ///
    /// 경로 문자열이면 `navigate_path`, 사용자 라우트 타입이면 그대로 넣는다.
    pub fn navigate<T: 'static>(&mut self, value: T) {
        self.navigations.push(Rc::new(value));
    }

    /// 문자열 경로 편의 함수 (`app.navigate("/users/42")`).
    pub fn navigate_path(&mut self, path: &str) {
        self.navigate(path.to_string());
    }

    pub fn take_navigations(&mut self) -> Vec<Rc<dyn Any>> {
        std::mem::take(&mut self.navigations)
    }

    /// Mockable stream: `mock_stream!`이 등록한 값을 우선 사용한다 (사양서 8.2).
    ///
    /// 첫 `pump()`에서 목을 확인하므로, mount 후에 `mock_stream!`을 등록해도 된다.
    pub fn spawn_mockable_stream<I, G>(
        &mut self,
        name: &'static str,
        real: impl FnOnce() -> I + 'static,
        k: G,
    ) where
        I: Iterator + 'static,
        I::Item: 'static,
        G: FnMut(&mut Arena, I::Item) + 'static,
    {
        let mut k = k;
        let mut real = Some(real);
        let mut it: Option<Box<dyn Iterator<Item = I::Item>>> = None;
        self.streams.push(Box::new(move |arena| {
            if it.is_none() {
                it = Some(match crate::runtime::take_stream_mock::<I::Item>(name) {
                    Some(values) => Box::new(values.into_iter()),
                    None => Box::new(real.take().expect("stream source")()),
                });
            }
            match it.as_mut().expect("stream task").next() {
                Some(v) => {
                    k(arena, v);
                    true
                }
                None => false,
            }
        }));
    }

    /// Spawn an effect: poll the future to completion, then run the
    /// continuation with the arena (사양서 5.1 — 런타임이 스폰·폴링·재디스패치).
    /// The effect is due immediately (`flush()`가 실행).
    pub fn spawn<F, G>(&mut self, fut: F, k: G)
    where
        F: Future + 'static,
        F::Output: 'static,
        G: FnOnce(&mut Arena, F::Output) + 'static,
    {
        self.spawn_after(0, fut, k);
    }

    /// Spawn a delayed effect: `targets <- f() after 300ms` (사양서 5.2 —
    /// `after` 접미사). `flush()`는 아직 due가 아닌 효과를 실행하지 않고,
    /// `advance(ms)`가 시계를 전진시켜 실행한다.
    pub fn spawn_after<F, G>(&mut self, delay_ms: u64, fut: F, k: G)
    where
        F: Future + 'static,
        F::Output: 'static,
        G: FnOnce(&mut Arena, F::Output) + 'static,
    {
        let due = self.now + delay_ms;
        self.pending.push(Pending {
            due,
            run: Box::new(move |arena| {
                let out = crate::runtime::block_on(fut);
                k(arena, out);
            }),
        });
    }

    /// Number of pending effects (due or not).
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Take every pending effect, regardless of due time.
    pub fn take_pending(&mut self) -> Vec<PendingEffect> {
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(|p| p.run)
            .collect()
    }

    /// Take only the effects whose due time has arrived, keeping the rest
    /// queued. This is what `flush()` and `advance()` call.
    pub fn take_due(&mut self) -> Vec<PendingEffect> {
        let now = self.now;
        let mut due = Vec::new();
        let mut rest = Vec::new();
        for p in std::mem::take(&mut self.pending) {
            if p.due <= now {
                due.push(p.run);
            } else {
                rest.push(p);
            }
        }
        self.pending = rest;
        due
    }

    /// Is an effect queued for later (`<- f() after 300ms`)?
    pub fn has_deferred(&self) -> bool {
        self.pending.iter().any(|p| p.due > self.now)
    }

    /// Register a stream: each yielded value runs the continuation with the
    /// arena (사양서 5.4 — 스트림을 슬롯에 반영).
    pub fn spawn_stream<I, G>(&mut self, iter: I, mut k: G)
    where
        I: Iterator + 'static,
        I::Item: 'static,
        G: FnMut(&mut Arena, I::Item) + 'static,
    {
        let mut it = iter;
        self.streams.push(Box::new(move |arena| match it.next() {
            Some(v) => {
                k(arena, v);
                true
            }
            None => false,
        }));
    }

    /// Number of live stream tasks.
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Take all live stream tasks (used by `pump`).
    pub fn take_streams(&mut self) -> Vec<StreamTask> {
        std::mem::take(&mut self.streams)
    }

    /// Put stream tasks back (survivors of a pump round).
    pub fn push_streams(&mut self, tasks: Vec<StreamTask>) {
        self.streams.extend(tasks);
    }
}

/// 슬롯 주소: 지역 슬롯(인덱스) 또는 store 필드(이름).
#[derive(Clone, Copy)]
enum SlotKey {
    Index(usize),
    Store(&'static str),
}

/// Copy handle to a state slot of type `T`.
pub struct State<T> {
    key: SlotKey,
    _pd: PhantomData<fn() -> T>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for State<T> {}

impl<T: 'static> State<T> {
    pub fn get<'a>(&'a self, arena: &'a Arena) -> &'a T {
        match self.key {
            SlotKey::Index(idx) => arena.get(idx),
            SlotKey::Store(k) => arena.store_get(k),
        }
    }
    pub fn set(&self, arena: &mut Arena, value: T) {
        match self.key {
            SlotKey::Index(idx) => arena.set(idx, value),
            SlotKey::Store(k) => arena.store_set(k, value),
        }
    }
    pub fn mutate<F: FnOnce(&mut T)>(&self, arena: &mut Arena, f: F) {
        match self.key {
            SlotKey::Index(idx) => arena.mutate(idx, f),
            SlotKey::Store(k) => arena.store_mutate(k, f),
        }
    }
    /// store 필드면 버전 카운터 (사양서 4.2), 지역 슬롯이면 항상 0.
    pub fn version(&self, arena: &Arena) -> u64 {
        match self.key {
            SlotKey::Store(k) => arena.store_version(k),
            SlotKey::Index(_) => 0,
        }
    }
}

/// Render context: 아레나 + 인스턴스 경로(키) + 프레임 구독 핸들러.
pub struct Ctx {
    pub arena: Arena,
    /// 부모가 넘겨준 슬롯 베이스 오프셋 — 인스턴스 경로의 일부가 된다.
    pub base: usize,
    /// Simulated clock in ms — advanced by `TestApp::advance` (사양서 8.2).
    pub now: u64,
    /// Keyboard handlers registered by `on_key` during the last render.
    pub keys: Vec<(String, KeyHandler)>,
    /// 인스턴스 경로 (사양서 9.5): `"/item42/@0"` 처럼 조립된다.
    path: String,
    /// 이번 프레임에 그려진 인스턴스 경로들.
    frames: HashSet<String>,
    /// 지난 프레임의 인스턴스 경로들 (unmount 감지).
    prev_frames: HashSet<String>,
    /// 형제 순번 — 키 없는 자식 컴포넌트의 경로 세그먼트(`@0`, `@1`, …).
    child_seq: usize,
    /// 키 스코프에 들어갈 때 저장한 부모 순번.
    key_seq: Vec<usize>,
    /// 경로별 `on_unmount` 핸들러.
    unmounts: HashMap<String, KeyHandler>,
    /// `on_event(Name)` 핸들러 (프레임마다 재등록).
    events: Vec<(String, KeyHandler)>,
    /// `on_net_change` 핸들러.
    net: Vec<KeyHandler>,
    /// `on_navigate` 핸들러.
    navs: Vec<NavHandler>,
}

impl Ctx {
    pub fn new() -> Self {
        Ctx {
            arena: Arena::new(),
            base: 0,
            now: 0,
            keys: Vec::new(),
            path: String::new(),
            frames: HashSet::new(),
            prev_frames: HashSet::new(),
            child_seq: 0,
            key_seq: Vec::new(),
            unmounts: HashMap::new(),
            events: Vec::new(),
            net: Vec::new(),
            navs: Vec::new(),
        }
    }

    /// 컴포넌트 매개변수·라이프사이클 슬롯. 인스턴스 경로가 키가 된다 (사양서 9.5).
    pub fn slot<T: 'static>(&mut self, idx: usize, init: impl FnOnce() -> T) -> State<T> {
        let key = format!("{}#{}", self.path, idx);
        self.arena.keyed_slot(&key, init)
    }

    /// store 전역 상태 (사양서 4.2).
    pub fn store<T: 'static>(&mut self, key: &'static str, init: impl FnOnce() -> T) -> State<T> {
        self.arena.store(key, init)
    }

    /// 현재 인스턴스 경로.
    pub fn instance_path(&self) -> &str {
        &self.path
    }

    /// 이번 프레임 시작: 베이스/핸들러/경로 초기화.
    pub fn begin_frame(&mut self) {
        self.base = 0;
        self.keys.clear();
        self.events.clear();
        self.net.clear();
        self.navs.clear();
        self.frames.clear();
        self.path.clear();
        self.child_seq = 0;
        self.key_seq.clear();
    }

    /// 자식 인스턴스의 경로 세그먼트를 확정한다 (사양서 9.5).
    ///
    /// 키가 있으면 키, 없으면 형제 순번 — **타입 이름**을 함께 넣어
    /// 서로 다른 컴포넌트가 같은 자리를 차지해도 식별자가 겹치지 않는다.
    /// (props/children을 만들기 **전에** 호출해야 경로가 밀리지 않는다.)
    pub fn instance_segment(&mut self, key: Option<String>, tag: &str) -> String {
        match key {
            Some(k) => format!("{}:{}", k, tag),
            None => {
                let s = format!("@{}:{}", self.child_seq, tag);
                self.child_seq += 1;
                s
            }
        }
    }

    /// 자식 인스턴스를 그린다: 경로 push → render → pop.
    /// `seg`는 `instance_segment`가 만든 세그먼트.
    pub fn with_instance<R>(&mut self, seg: String, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let before = self.path.len();
        let saved_seq = std::mem::replace(&mut self.child_seq, 0);
        self.path.push('/');
        self.path.push_str(&seg);
        self.frames.insert(self.path.clone());
        let out = f(self);
        self.path.truncate(before);
        self.child_seq = saved_seq;
        out
    }

    /// `<Tag key={…}>` 키 스코프 — 자식 컴포넌트의 슬롯 경로가 키를 포함한다.
    pub fn enter_key(&mut self, key: String) {
        self.path.push('/');
        self.path.push_str(&key);
        // 키 스코프 안에서는 형제 순번을 다시 센다 (`<Row key={id}>` 안의 자식)
        self.key_seq.push(self.child_seq);
        self.child_seq = 0;
    }

    pub fn exit_key(&mut self) {
        self.child_seq = self.key_seq.pop().unwrap_or(0);
        if let Some(i) = self.path.rfind('/') {
            self.path.truncate(i);
        }
    }

    /// 이번 프레임 끝: 사라진 인스턴스의 상태를 버리고 전달할 핸들러를 모은다.
    pub fn end_frame(&mut self) -> FrameTail {
        let current = std::mem::take(&mut self.frames);
        let mut tail = FrameTail::default();
        for path in self.prev_frames.difference(&current) {
            if let Some(h) = self.unmounts.remove(path) {
                tail.unmounts.push(h);
            }
            self.arena.drop_instance(path);
        }
        self.prev_frames = current;

        for name in self.arena.take_emitted() {
            for (n, h) in &self.events {
                if *n == name {
                    tail.events.push(h.clone());
                }
            }
        }
        if self.arena.take_online_changed() {
            tail.net.extend(self.net.iter().cloned());
        }
        for value in self.arena.take_navigations() {
            for h in &self.navs {
                tail.navs.push((Rc::clone(&value), h.clone()));
            }
        }
        tail
    }

    /// `on_unmount { … }` 등록 (현재 인스턴스 경로 기준).
    pub fn on_unmount(&mut self, h: KeyHandler) {
        self.unmounts.insert(self.path.clone(), h);
    }

    /// `on_event(Name) { … }` 등록.
    pub fn on_event(&mut self, name: &str, h: KeyHandler) {
        self.events.push((name.replace(' ', ""), h));
    }

    /// `on_net_change { … }` 등록.
    pub fn on_net_change(&mut self, h: KeyHandler) {
        self.net.push(h);
    }

    /// `on_navigate(|path| …)` 등록.
    pub fn on_navigate(&mut self, h: NavHandler) {
        self.navs.push(h);
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
