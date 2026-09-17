# elm-magic

> **"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**

Rust의 타입 시스템을 유지하면서, 매크로로 문법을 JS/JSX처럼 위장한
초경량 순수 함수형 UI 라이브러리. (사양서: `prototype/prototypes/spec.md`)
(구현 현황 · 미구현 목록: `prototype/prototypes/implementation-status.md`)

현재 버전: **0.5.0** — 변경 내역은 [`CHANGELOG.md`](CHANGELOG.md).

## 설치

```toml
[dependencies]
elm-magic = "0.5"
```

egui 위에 그리려면 어댑터 크레이트를 함께 추가한다:

```toml
[dependencies]
elm-magic = "0.5"
elm-magic-egui = "0.5"
```

## 의존성 정책 — 기본은 0, `serde`만 선택적

기본 빌드는 **외부 의존성 0**이다(`cargo tree -p elm-magic -e normal` → `elm-magic-macros`뿐).
사양서 8.3의 "뷰는 직렬화 가능"(`Element: Serialize`)은 선택 기능으로 제공한다:

```toml
[dependencies]
elm-magic = { version = "0.5", features = ["serde"] }
```

```sh
cargo test -p elm-magic --features serde   # tests/snapshot.rs
```

`Element`(과 `StyleProps`)가 `Serialize`를 얻는다. 핸들러(`Rc<dyn Fn>`)와 `<Raw>` 클로저는
`#[serde(skip)]`으로 빠지고 **구조·텍스트·클래스만** 직렬화되므로, `render_tree()`(사람용 텍스트)와
달리 기계가 비교하는 스냅샷(insta 등)에 쓸 수 있다.

**나머지 3종은 넣지 않는다** — 이 설계에는 맞지 않는다(근거는
`prototype/prototypes/implementation-status.md` 2.8절):

| 라이브러리 | 판단 | 근거 |
|---|---|---|
| `serde` | ✅ 선택 기능 | 사양서 8.3의 직렬화 계약. `#[cfg_attr(feature = "serde", …)]`로 기본 그래프를 건드리지 않음 |
| `thiserror` | ❌ | 공개 API가 **panic 기반**이다(매크로 사용 오류를 명확한 한국어 메시지로 즉시 실패). 반환형 `Result`가 없어 `#[derive(Error)]`를 넣어도 `panic!("{}", e)`로 감싸는 코드만 늘어난다 |
| `enum_dispatch` | ❌ | `Element`는 데이터 enum이고 핸들러는 **클로저**(고유 타입)라 dispatch 대상 트레이트가 없다. 도입하려면 variant를 구조체로 바꾸고 `testing.rs`/egui 어댑터/테스트의 패턴 매칭을 전부 갈아엎어야 하는데, 단순화가 아니라 순수 비용이다 |
| `tokio` | ❌ | `<-` 효과는 `Future + 'static`(**`Send` 아님**)이고 테스트는 동기(`flush`) 모델이다. tokio를 넣으면 `Send` 요구와 런타임이 따라온다. 현재 `runtime::block_on`은 noop waker 스핀이라 **진짜 I/O future에는 부적합**하지만, 그건 tokio가 아니라 설계 결정(="headless는 즉시 완료되는 future만")의 문제다 |

## 사용 예

```rust
use elm_magic::prelude::*;

elm_magic::view! {
    fn Counter(n = 0) {
        <Row>
            <Button on_click={n -= 1}>"-"</Button>
            "Count: {n}"
            <Button on_click={n += 1}>"+"</Button>
        </Row>
    }
}

fn main() {
    let mut app = elm_magic::mount!(Counter);
    app.click("+");
    app.expect_text("Count: 1");
}
```

## 다섯 가지 문법 (v0.1 구현 현황)

| 문법 | 의미 | 현황 |
|---|---|---|
| **변수** = 상태 | `fn Counter(n = 0)`의 매개변수가 아레나 슬롯으로 승격 | ✅ v0.1 |
| **대입** = 이벤트 | `on_click={n += 1}`이 슬롯 변경으로 컴파일 | ✅ v0.1 |
| **태그** = 뷰 | `Col` `Row` `Text` `Strong` `Button` `Input` `TextArea` `Check` `Tab` `Th` `Td` `Banner` `Spinner` `Divider` `Progress` `Modal` `Raw` (+`Fragment`) | ✅ v0.4 |
| **`<-`** = 효과 | `status, users <- fetch()`, `<- f() after 300ms` — 런타임이 스폰·폴링 | ✅ v0.2 · 지연 효과 v0.4 |
| **`mock`** = 테스트 | `app.mock(f, \|q\| …)` · `.mock2` · `mock!` · `flush()` · `advance(ms)` · `pump()` · `press_key()` | ✅ v0.3 / v0.4 |

라이프사이클(v0.2/v0.4): `on_mount { ... }`, `on_key("Ctrl+S") { ... }`,
`on_tick(500ms) { ... }`, `on_change(x) after 300ms { ... }`(디바운스),
`on_change(x) { ... }`(즉시 감시 — `x`는 `Clone + PartialEq` 필요). 스타일: `css!`.

플랫폼(v0.3): `elm_magic::run(Headless, App)` — 플랫폼은 인자. egui 어댑터는
별도 크레이트 `elm-magic-egui`(`render(ui, &tree, &mut arena)`).

## 구조

```
src/
├── lib.rs        # public API (Component, frame, mount!, prelude)
├── element.rs    # Element enum + IntoElements + Callback (조건부/이터레이터 통일)
├── state.rs      # Arena(지역 슬롯·store·keyed 경로), State<T> Copy 핸들, Ctx(프레임/구독)
├── runtime.rs    # 효과·스트림 목 레지스트리 (스레드 로컬)
└── testing.rs    # 헤드리스 TestApp (click / type_ / pump / emit / navigate / expect_text)
crates/elm-magic-macros/
├── src/jsx.rs    # JSX 토큰 변환기 (태그·이벤트·store·구독·콜백)
├── src/view.rs   # view! — props Option<T>, 콜백/children 사전 바인딩, 슬롯
├── src/store.rs  # #[store] / store_fn! — 접근자 + proc-macro 레지스트리
└── src/lib.rs    # 매크로 진입점
tests/            # counter todo effects lifecycle css mock stream raw platform
                  # syntax timers widgets sugar store subs callbacks
                  # integration bug_report (elm-magic-bug-report.md 회귀)
                  # snapshot (`--features serde` — 사양서 8.3 직렬화 계약)
```

## 테스트

```sh
cargo test                                  # 기본 — 의존성 0, 96개
cargo test --workspace --all-features       # serde 스냅샷 포함, 100개
```

- 렌더러도 런타임도 없이 순수 함수 호출만으로 UI 로직을 검증한다 (사양서 8.1).
- `render_tree()`는 Element 트리의 텍스트 덤프 — 스냅샷 계약 (사양서 8.3).

## 매핑 (사양서 10)

| 사용자 문법 | 전개 |
|---|---|
| `fn Counter(n = 0)` | `CounterProps { n: i32 }` + `ctx.slot(0, \|\| props.n.clone())` |
| `on_click={n += 1}` | `Rc::new(move \|_elm_a\| { state.mutate(arena, \|v\| *v += 1) })` |
| `"Count: {n}"` | `Element::Text { text: format!("Count: {}", n) }` |
| `{items.map(\|t\| <Row>...)}` | `into_elements((state.clone()).into_iter().map(...))` — 소유 반복 |
| `{if x { A } else { B }}` | 분기마다 `into_elements(..)` → `Vec<Element>` 통일 (본문이면 `into_element` → `Fragment`) |
| `<Counter start=0 />` | 인라인 렌더 + 슬롯 베이스 오프셋 (`SLOTS`) |

## v0.5 — 전역 상태 · keyed 트리 · 구독 · 콜백 prop

### `#[store]` 전역 상태 (사양서 4.2)

```rust
use elm_magic::prelude::*;

#[store]
struct App { count: i32, dark: bool }     // 기본값은 `Default::default()` (i32→0, bool→false)

// 필드 기본값을 쓰고 싶으면 함수형 `store_fn!`
elm_magic::store_fn! { Settings { level: i32 = 3, muted: bool = true } }

elm_magic::view! {
    fn Header() {
        <Row>
            "count: {app.count}"                                  // 어디서든 app.field
            <Button on_click={app.count += 1}>"inc"</Button>       // 대입/증감도 그대로
            <Button on_click={app.dark = !app.dark}>"theme"</Button>
        </Row>
    }
}
```

- 슬롯은 아레나에 **이름 기반**(`"App.count"`)으로 저장 → 컴포넌트 사이에 공유.
- `State::version(&arena)`으로 변경 추적(버전 카운터).
- `#[store]`는 **사용하는 `view!`보다 위에** 선언한다 (proc-macro 레지스트리).
- 주의: `#[store] struct X { a: i32 = 0 }` 문법은 rustc가 아직 불안정(필드 기본값)이라
  속성 매크로에서는 쓸 수 없다 → `store_fn!`을 쓴다.

### keyed 트리 · unmount (사양서 9.5)

```rust
elm_magic::view! {
    fn ItemList(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <Row key={id}><Item id={id} /></Row>)}   // key = 인스턴스 경로
            <Button on_click={ids.reverse()}>"reverse"</Button>
        </Col>
    }
}

elm_magic::view! {
    fn Session() {
        on_unmount { app.closed += 1 }    // 사라질 때 실행
        <Text>"session open"</Text>
    }
}
```

- 슬롯 키 = **인스턴스 경로**(`/<key 또는 @순번>:<컴포넌트>#<슬롯>`) → 순서가 바뀌어도 상태가 따라간다
  (타입 이름이 경로에 들어가므로 자리가 바뀐 다른 컴포넌트와 식별자가 겹치지 않는다).
- 사라진 경로의 슬롯은 **초기화**되고 `on_unmount`가 실행된다 (재마운트 시 상태 0부터).
- key가 없어도 형제 순번으로 분리되므로 `<Col>` 안의 리스트 컴포넌트들이 슬롯을 섞지 않는다.

### 구독 (사양서 5.3, 5.4)

```rust
elm_magic::view! {
    fn Chat(room = String::from("lobby"), msgs: Vec<String> = vec![]) {
        on_message(messages(room.clone()), m) { msgs.push(m); }   // 스트림 구독(인스턴스당 1회)
        <Col>{msgs.map(|m| <Row>"{m}"</Row>)}</Col>
    }
}

elm_magic::view! {
    fn Toolbar() { <Button on_click={bus.emit(RefreshRequested)}>"refresh"</Button> }

    fn DataTable(items: Vec<String> = vec![], hits = 0) {
        on_event(RefreshRequested) { hits += 1 }                  // 이벤트 버스
        on_net_change { online = net.is_online() }                // 네트워크 상태
        on_navigate(|path| route = path)                          // 라우팅 (String 또는 라우트 타입)
        <Col>"hits: {hits}"</Col>
    }
}
```

테스트에서 구동:

```rust
elm_magic::mock_stream!(app, messages, ["m1".to_string()]);  // 스트림 목
app.pump();                    // 스트림 한 값
app.emit("RefreshRequested");  // 이벤트 버스
app.set_online(false);         // on_net_change
app.navigate("/users/42");     // 경로(String)
app.navigate_value(Route::User(42));  // 타입 있는 라우트
```

- `on_message(소스, 이름)` — 소스는 **실제 이터레이터/함수 호출**이어야 한다(서비스 객체 미구현).
  `mock_stream!`은 첫 `pump()`에서 확인하므로 mount 후에 등록해도 된다.
- `on_event(Name)` / `bus.emit(Name)`은 이름을 `stringify!`로 정규화해 맞춘다.

### 콜백 prop + 컴포넌트 children (사양서 3.1)

```rust
elm_magic::view! {
    fn ItemRow(item: Item, on_select: fn(i32)) {        // fn(T) = 콜백 prop
        <Row on_click={on_select(item.id)}>"{item.name}"</Row>
    }

    fn List(items: Vec<Item> = vec![], selected = 0) {
        <Col>
            {items.map(|i| <ItemRow item={i} on_select={selected = _} />)}
            "selected: {selected}"
        </Col>
    }

    fn Card(title = String::new()) {
        <Col>"card: {title}"{children}</Col>            // 자식 prop
    }

    fn Page() { <Card title="hi"><Text>"body"</Text></Card> }
}
```

- prop은 전부 `Option<T>` — `None`이면 선언된 기본값, 필수 prop(`item: Item`)은
  `mount!` 시 명확한 panic 메시지.
- `on_*={...}`는 컴포넌트에서 **콜백 prop**으로 전개된다(`Callback<T>`, 호출부에서 클로저 생성).
- `<Row on_click={…}>`처럼 컨테이너도 클릭 가능(헤드리스 `click`, egui 어댑터 모두).
- 문자열 보간은 `{x}` 외에 `{x:?}`, `{x:.2}` 같은 서식 지정자도 지원.

## v0.4 — 문법 픽스 + 어휘 확장

버그 픽스로 사양서 3.3/3.4(리스트, 조건부)가 그대로 돌아간다.

```rust
elm_magic::view! {
    fn Todos(items: Vec<Todo> = vec![], text = String::new(), filter = String::new()) {
        on_change(text) after 300ms { }                 // 시간 리터럴
        <Col>
            <Input value={text.clone()} on_change={text = _}
                   on_enter={items.push(Todo { text: text.clone(), done: false })} />
            // 구조체 축약 · 요소로 삭제 · 컴마 다중문 · 반환값 무시
            <Button on_click={items.push(Todo { text: text.clone(), done: false }), filter = String::new()}>"Add"</Button>
            <Button on_click={items.remove(0)}>"pop"</Button>
            // 분기마다 타입이 달라도 됨 (요소 / 이터레이터 / 문자열)
            {if items.is_empty() { <Banner kind="info">"empty"</Banner> } else {
                items.map(|t| <Row>
                    <Check checked={t.done} on_change={t.done = !t.done}>{t.text}</Check>
                    <Button on_click={items.remove(t)}>"x"</Button>   // 아이템 캡처 OK
                </Row>)
            }}
            {match filter.as_str() {
                "" => <Text>"all"</Text>,
                f => <Strong>"filter: {f}"</Strong>,
            }}
            {(0..items.len()).map(|i| <Row>"{i}"</Row>)}          // `..` 뒤 상태 읽기
        </Col>
    }
}
```

- **새 태그**: `Spinner`, `Divider`, `Strong`, `Banner`, `Check`, `TextArea`, `Tab`, `Th`, `Td`, `Progress`,
  `<Modal>`(자식 보유). 헤드리스 텍스트 추출·덤프·egui 어댑터까지 매핑.
- **새 규칙**:
  - `{_}`는 값 이벤트(`on_change` / `on_enter`)에서만 — 아니면 명확한 컴파일 에러.
  - `state.remove(정수)`는 인덱스 삭제, `state.remove(그 밖)`은 **값으로 삭제**(`PartialEq` 필요).
  - `items.iter()` / `items.map(…)`는 **소유 반복**(`into_iter`)으로 전개 — 리스트 아이템을
    이벤트 핸들러에 넘길 수 있다(사양서 3.3의 "`&` 생략").
  - 뷰 본문/분기의 값이 `Vec<Element>`면 내부적으로 `Fragment`로 감싼다.
- **테스트 슈가**: `app.mock(fn, |q| …)` / `.mock2` / `.mock3`, `assert_text` / `assert_visible` / `assert_hidden`,
  `type_into("input"|"textarea"|클래스, 값)`, `toggle(label)`, `set_check(label, v)`, `has_pending_after()`.
- **지연 효과**: `on_mount { fresh <- api_user(id) after 5min }` — `flush()`는 due가 된 효과만 실행하고,
  `app.advance(ms)`가 시계를 밀어 지연 효과를 발화시킨다.

## v0.3 — 플랫폼 (사양서 7, 5.4, 8.2)

```rust
// 진입점: 플랫폼은 인자 — 컴포넌트는 egui/웹/터미널을 모른다
let mut app = elm_magic::run(elm_magic::Headless, App);

// 스트림: 스트림이 내보내는 값마다 슬롯에 반영 (사양서 5.4)
<Button on_click={ upload_progress() -> pct { } }>"upload"</Button>
// 테스트: app.pump()로 한 값씩 펌핑

// 모킹: 효과 함수를 런타임에 교체 (사양서 8.2)
elm_magic::mock!(app, search_api, |q: String| vec![format!("hit:{}", q)]);
app.flush();

// <Raw>: 유일한 탈출구 (사양서 7.3) — 헤드리스에선 무시, 어댑터가 호출
<Raw>|ui: &mut egui::Ui| { ui.hyperlink_to("docs", "https://…"); }</Raw>
```

egui 어댑터(`crates/elm-magic-egui`)는 실제 egui 입력 이벤트로 클릭을
시뮬레이션하는 통합 테스트를 갖는다 — 같은 컴포넌트가 헤드리스와 egui에서
동일하게 동작함을 검증.

## v0.2 — 효과 (사양서 5.1, 5.2)

```rust
elm_magic::view! {
    fn Users(users: Vec<String> = vec![], status = String::from("idle")) {
        on_mount { status, users <- load_users() }          // 마운트 시 로드
        <Col>
            <Button on_click={status, users <- load_users()}>"Load"</Button>
            on_key("Ctrl+R") { status, users <- load_users() } // 키보드
            on_change(status) after 300 { }                    // 디바운스 감시
            on_tick(1000) { }                                  // 티커
            "status: {status}"
            {users.map(|u| <Row>"{u}"</Row>)}
        </Col>
    }
}
```

- `<-`는 "이 future의 결과를 이 슬롯들에 쓴다" — `Pin`/`Box`/`Send` 노출 없음.
- Cmd는 flush 전까지 실행 안 됨: `app.flush()`로 대기 효과 실행 (사양서 12).
- 시뮬레이션 시계: `app.advance(ms)`로 `on_tick`/`on_change after` 구동.
- 키보드: `app.press_key("Ctrl+S")`.

## v0.1 제한

- 속성 매크로 `#[view]` 대신 함수형 매크로 `view! { fn ... }` 사용
  (매개변수 기본값 `n = 0`은 rustc 파서가 거부하는 비-Rust 문법이므로
  토큰을 원본 그대로 받는 함수형 매크로가 필요).
- 중첩 컴포넌트의 상태 슬롯은 렌더 순서 기반 오프셋으로 분리
  (조건부로 등장하는 컴포넌트 순서가 바뀌면 슬롯이 섞일 수 있음 — keyed 트리는 v0.4+).

## v0.3 제한

- `mock!`은 클로저 매개변수에 타입 어노테이션이 필요
  (`|q: String| ...`) — 다운캐스트 키로 사용.
  v0.4부터는 사양서식 `app.mock(fn, |q| ...)`도 가능(함수명을 타입 이름에서 추출).
- 효과 목은 `<-`의 단순 호출 `f(args)` 형태만 지원(메서드 호출·복합식은 미지원),
  스트림(`->`) 목은 미지원. 0-인자 효과 함수는 목 불가.
- `mock`은 스레드 로컬 — 각 테스트는 독립 스레드에서 격리됨.
- `css!` 자기등록은 `.init_array` ctor — Linux/macOS 동작, wasm은 v1.0 과제.
- `on_message` / `on_event` / `on_navigate` 구독과 `#[store]`는 아직 미구현.

## v0.4 제한

- `on_change(x)`의 `x`는 `Clone + PartialEq` (변화 감지를 위해 값 비교).
- `{_}`는 값 이벤트(`on_change`/`on_enter`)와 **컴포넌트 콜백 prop**에서만 — `on_click` 등에는 전달값이 없다.
- `state.remove(비정수)`는 `Vec::retain` + `PartialEq`로 전개된다(`items.remove(t)` = 값으로 삭제).
- `items.iter()` / `items.map(…)`는 소유 반복(`into_iter`)이므로 아이템을 수정하려면
  `items` 쪽 메서드를 쓰거나 다시 `push`해야 한다 (`&mut` 이터레이션은 사양서 대상 아님).
- `Table`, `VirtualList`, `Scroll`, `Plot`, `Dock`, `Window`, `Sidebar`, `Menu/Item`, `Fade` 등 어휘와
  `Desktop` / `Web` / `Terminal` 플랫폼은 v0.6+ 과제 (egui 어댑터는 현재 17개 태그 매핑).
- **리스트 아이템 필드 대입은 아직 안 된다**: `items.map(|t| …)`의 `t`는 값 복사본이므로
  `on_change={t.done = !t.done}` 같은 대입은 `E0594`가 난다. 대신 `items` 쪽 메서드를 쓴다
  (예: `<Check checked={t.done} on_change={items.toggle(&t)}>` — 아이템 편집/인덱스 추적은 다음 과제).
- 한 행에서 **같은 아이템을 캡처하는 핸들러가 2개 이상**이면 `move` 캡처가 충돌한다.
  핸들러는 아이템당 하나만 두고, 나머지는 읽기(텍스트/`checked`)로 표현한다.

## v0.5 제한

- `#[store]` 필드 기본값은 `Default::default()` (속성 매크로 입력은 rustc가 구조체로 파싱하므로
  필드 기본값 문법을 쓸 수 없다) → 명시적 기본값은 `store_fn!` 형태.
- `#[store]`는 사용하는 `view!`보다 **위에** 선언해야 인식된다 (proc-macro 레지스트리).
- `on_message(소스, …)`의 소스는 실제 이터레이터(함수 호출/변수)여야 한다 —
  `ws`, `api` 같은 **서비스 객체는 미구현**. 목을 걸어도 실제 소스가 컴파일되어야 한다.
- `on_event(Name)`은 `Name`을 문자열로 정규화해 맞춘다(`bus.emit`과 동일 규칙).
- `on_navigate` 핸들러가 받는 값은 `String`(경로) 또는 `navigate_value`로 넣은 라우트 타입 —
  타입이 다르면 런타임 panic.
- 구독(`on_message`)은 **인스턴스당 한 번**만 등록된다(재구독은 unmount 후 재마운트로).
- keyed 경로는 문자열(`"/<key>/@<n>#<slot>"`)이다 — 성능 최적화(해시/u32 경로)는 다음 과제.

디버깅: `ELM_MAGIC_DUMP=1 cargo build`로 `view!` / `store` 전개 코드를 그대로 볼 수 있다
(사양서 13장의 "`cargo expand` 필수" 항목 대체).

## v0.5 버그 픽스 — 리포트 3건 (테스트 우선)

`elm-magic-bug-report.md`의 3건을 재현 테스트(`tests/bug_report.rs`, 6개)로 먼저 고정한 뒤 수정했다.

| # | 증상 | 원인 | 수정 |
|---|---|---|---|
| 1 | `pub fn` / `pub(crate) fn` 컴포넌트가 `visibility pub is not followed by an item` | `vis`가 `#[derive(..)]` **앞**에도 찍혀 `pub #[derive(..)]`가 되었고, `pub(crate)` 그룹은 버려졌다 | `vis`는 `struct` 앞에만, `pub(crate)`/`pub(super)`/`pub(in path)` 그룹 내용까지 보존 (`view.rs`, `store.rs` 동일 수정) |
| 2 | `on_click={items.remove(x), n = 1}`이 `expected expression, found ','` | `remove(x)`(`retain`) 특수 폼 전개가 문장 종료자(`;`/`,`)를 소비하지 않아 `,`가 그대로 남았다 | 특수 폼 뒤에서 `;`/`,`를 소비하고 `;`로 재방출 (`jsx.rs::transform_event`) |
| 3 | `{if ..}` 식 안 지역 컬렉션의 `.iter().map(..)`에서 `E0716` | 슈가가 **상태 슬롯**만 소유 반복으로 전개하고 지역 `let` 변수는 건드리지 않았다 | 상태가 아닌 식별자의 `x.iter().map(..)`도 `(x).clone().into_iter().map(..)`로 전개 (`jsx.rs::local_iter_sugar`) |

```rust
// 이제 모두 그대로 컴파일·동작한다
elm_magic::view! {
    pub fn Badge(count = 0) { <Row>"count: {count}"</Row> }
}

elm_magic::view! {
    fn T(items: Vec<String> = vec![], n = 0) {
        // `remove(x)` 뒤에 다른 문장이 와도 된다 (버그 2)
        <Button on_click={items.remove(String::from("a")), n = 1}>"go"</Button>
    }
}

elm_magic::view! {
    fn S(open = true, status = String::new()) {
        let sections = ["Notes", "PDFs"];
        <Col>
            {if open {
                {sections.iter().map(|s| <Row on_click={status = format!("{}", s)}>"{s}"</Row>)}
            } else { <Text>"none"</Text> }}
            "status: {status}"
        </Col>
    }
}
```

여전히 남은 v0.5 제한(아이템 필드 대입 `t.done = !t.done`, 아이템당 핸들러 2개 이상)은 그대로다.

전체 현황: `prototype/prototypes/implementation-status.md`
변경 내역: `CHANGELOG.md`

