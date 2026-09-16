# elm-magic — 사양서

> **"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**

---

## 1. 철학

### 1.1 다섯 가지 원칙

| 원칙 | 의미 |
|---|---|
| **순수 함수** | `view`는 상태의 함수. 같은 상태 → 같은 트리. |
| **플랫폼 중립** | 컴포넌트는 egui/웹/터미널을 모른다. 렌더러는 `main`에서 주입. |
| **테스터빌리티** | UI 없이, 렌더러 없이, 매크로 없이 로직 검증 가능. |
| **Not Rust-like** | 사용자 코드에 제네릭·라이프타임·트레잇 바운드·`unsafe`가 없다. |
| **초경량** | `Element`는 `enum`, `Msg`도 `enum`. 핫 패스에 `dyn` 없음. |

### 1.2 핵심 통찰

> **"사람이 이해하기 쉬워야 기계도 이해하기 쉽다."**

- 문법은 JS/JSX처럼 보이게.
- 컴파일 결과는 정적 Rust.
- 복잡함(제네릭, async 배관, 아레나, unsafe)은 전부 매크로 뒤에 숨긴다.

### 1.3 표면적 문법은 다섯 개뿐

1. **변수** = 상태 (`n = 0`)
2. **대입** = 이벤트 (`n += 1`)
3. **태그** = 뷰 (`<Col>`, `<Button>`)
4. **`<-`** = 효과 (async, 스트림, 스폰)
5. **`mock()`** = 테스트

이 다섯 개의 조합으로 모든 UI를 표현한다.

---

## 2. 설치

```toml
[dependencies]
elm-magic = "0.1"
```

```rust
use elm_magic::prelude::*;
```

---

## 3. 사용자 문법

### 3.1 컴포넌트

```rust
#[view]
fn Counter(n = 0) {
    <Col>
        "Count: {n}"
        <Row>
            <Button on_click={n += 1}>"+"</Button>
            <Button on_click={n -= 1}>"-"</Button>
        </Row>
    </Col>
}
```

- `#[view]` 매크로가 컴포넌트를 정의.
- 매개변수는 곧 상태. 기본값을 가질 수 있음.
- 함수 본문이 곧 UI.

### 3.2 태그

```rust
<Col gap=8>
    <Text class="title">"Hello"</Text>
    <Input value={text} on_change={text = _} />
    <Button disabled={!valid} on_click={submit()}>"Save"</Button>
</Col>
```

- HTML/JSX 유사 문법.
- 속성은 `key=value` 또는 `key={표현식}`.
- `{_}`는 "이 값 자체"를 의미 (`text = _`는 `text = value`).
- `on_click={n += 1}`은 대입을 이벤트로 승격.

### 3.3 리스트

```rust
{items.map(|t| ui! {
    <Row>
        <Check checked={t.done} on_change={t.done = !t.done} />
        "{t.text}"
        <Button on_click={items.remove(t)}>"x"</Button>
    </Row>
})}
```

- 이터레이터가 곧 자식.
- `.iter()` / `&` 생략 가능 (매크로가 처리).
- keyed diffing은 자동.

### 3.4 조건부

```rust
{if loading {
    <Spinner />
} else if let Some(e) = &error {
    <Banner kind="error">{e}</Banner>
} else {
    items.map(|i| <Row>{i.name}</Row>)
}}
```

- `if` / `else if` / `else if let` / `match` 모두 표현식.
- 각 분기가 `Element` / `Vec<Element>` / `Option<Element>` 반환.
- `IntoElements` 트레잇이 통일 (derive로 숨김).

---

## 4. 상태 모델

### 4.1 지역 상태

```rust
#[view]
fn Counter(n = 0) { ... }
```

- 매개변수가 슬롯으로 승격.
- 매크로가 `n`을 `slot_index = 0`으로 치환.
- `state()` 호출도, 훅 순서 규칙도 없음.
- 조건문 안에서 상태 선언 불가 (컴파일 에러).

### 4.2 전역 상태

```rust
#[store]
struct App { user: Option<User>, theme: Theme }

#[view]
fn Header() {
    "Hello, "{app.user.name}
    <Button on_click={app.theme = app.theme.toggle()}>"theme"</Button>
}
```

- `#[store]`가 슬롯 배열 + 타입 ID 기반 저장소 생성.
- 어디서든 `app.field`로 접근.
- `Rc<RefCell>` 없음. `Arc<Mutex>` 없음.
- 변경 추적은 버전 카운터.

### 4.3 파생 값

```rust
let total = items.iter().map(|i| i.price * i.qty).sum();
let tax   = total * 0.1;
```

- 그냥 지역 변수.
- 캐시 없음, 메모이제이션 없음.
- 매 렌더 재계산. 필요하면 `memo!` 매크로.

### 4.4 상태의 정체

내부적으로 상태는 **아레나 슬롯**:

```rust
// 개념적 전개
let n: State<i32> = ctx.slot(0, || 0);
```

- `State<T>`는 슬롯 인덱스 핸들 (`Copy`).
- `Deref`/`DerefMut`로 `n += 1` 지원.
- 안전성은 아레나가 보장. `unsafe` 한 줄이 라이브러리 안에 숨음.

---

## 5. 효과 (Effects)

### 5.1 비동기

```rust
<Button on_click={status, servers <- fetch_servers()}>"Refresh"</Button>
```

- `<-`는 "이 future의 결과를 이 슬롯들에 쓴다".
- 런타임이 스폰·폴링·재디스패치.
- `Pin` / `Box` / `Send` / `'static` 노출 없음.
- 실패 시 자동 되돌림: `rollback <- api.toggle(id)`.

### 5.2 라이프사이클

```rust
on_mount { users <- api.load() }
on_unmount { ws.close() }
on_tick(16ms) { samples.push(read_sensor()) }
on_change(query) after 300ms { results <- search(query) }
```

### 5.3 구독

```rust
on_message(ws, m) { msgs.push(m) }
on_event(RefreshRequested) { items <- api.items() }
on_net_change { online = net.is_online() }
on_key("Ctrl+S") { save(text) }
on_navigate(|r| route = r)
```

### 5.4 스트림

```rust
upload(f) progress -> u { files[u].progress = pct }
```

- 스트림을 슬롯에 반영.
- 백프레셔는 런타임이 처리.

---

## 6. 스타일

### 6.1 Tailwind-like CSS

```rust
css! {
    .app    { gap: 16; padding: 24; }
    .card   { gap: 8;  padding: 16; bg: surface; radius: 8; }
    .muted  { color: text_dim; }
    button  { bg: primary; color: on_primary; padding: 8 16; radius: 6; }
    h1      { font-size: 24; weight: bold; }
}
```

- 클래스는 문자열 리터럴.
- 내부적으로 `StyleId(u32)`로 인터닝.
- 렌더 시 정수 비교.

### 6.2 인라인 스타일

```rust
<Text class={if error { "error" } else { "ok" }}>"..."</Text>
```

- 조건부 스타일은 값.
- 상태를 바꾸지 않음.

### 6.3 테마

```rust
<Theme preset="dark">
    <App />
</Theme>
```

- 테마는 context.
- `#[store]`와 동일한 메커니즘.

---

## 7. 플랫폼 추상화

### 7.1 진입점

```rust
fn main() {
    elm_magic::run(Desktop, App);
    // elm_magic::run(Web, App);
    // elm_magic::run(Terminal, App);
    // elm_magic::run(Headless, App);
}
```

- `Platform`은 인자.
- 컴포넌트는 플랫폼을 모름.
- 같은 `App`이 네 곳에서 그대로 실행.

### 7.2 어댑터

| 클라이언트 | 어댑터 |
|---|---|
| `<Sidebar>` | `egui::SidePanel` |
| `<Scroll>` | `egui::ScrollArea` |
| `<Plot>` | `egui_plot::Plot` |
| `<Table virtualized>` | `oxiui-table` |
| `<Dock>` | `egui_dock::DockState` |
| `<Window>` | `egui::Window` |

어댑터는 라이브러리 내부. 클라이언트는 이름조차 모름.

### 7.3 탈출구

```rust
<Raw>|ui: &mut egui::Ui| {
    ui.hyperlink_to("docs", "https://...");
}</Raw>
```

- 유일한 escape hatch.
- egui 특수 위젯이 필요할 때만.
- 헤드리스 테스트에서는 무시됨.

---

## 8. 테스트

### 8.1 헤드리스

```rust
#[test]
fn counter_increments() {
    let app = mount(ui! { <Counter /> });
    app.click("+");
    app.expect_text("Count: 1");
}
```

- `mount!`가 인스턴스 트리 생성.
- egui Context 불필요.
- 런타임 불필요.
- 순수 함수 호출.

### 8.2 모킹

```rust
#[test]
fn search_debounce() {
    let app = Search().mock(search_api, |q| vec![Hit { title: format!("hit:{q}") }]);
    app.type_("input", "rust");
    app.advance(300ms);
    app.assert_text("hit:rust");
}
```

- `.mock(fn, impl)` — 런타임 교체.
- `advance(duration)` — 시간 진행.
- `flush()` — pending Cmd 실행.

### 8.3 스냅샷

```rust
#[test]
fn root_snapshot() {
    insta::assert_snapshot!(mount(ui! { <App /> }).render_tree());
}
```

- `Element` 트리는 순수 데이터 → 직렬화 가능.
- 스냅샷이 곧 계약.

---

## 9. 성능 모델

### 9.1 정적 디스패치

- `Element<Msg>`는 `enum`.
- `Msg`도 `enum`.
- `match`는 점프 테이블로 컴파일.
- 핫 패스에 `Box<dyn Trait>` 없음.

### 9.2 메모리

- 트리는 **bump arena**에 매 프레임 할당.
- 프레임 끝에 통째로 리셋.
- `Vec<Element>`는 **SmallVec**으로 대부분 스택.
- 문자열은 **인터닝** (`"primary"` → `StyleId(3)`).

### 9.3 상태

- `State<T>`는 슬롯 인덱스 핸들.
- `Rc`도 `RefCell`도 없음.
- `Deref`/`DerefMut`는 아레나가 보장.

### 9.4 효과

- async future는 상태 머신으로 단형화.
- 힙 할당은 `Box::pin` 한 번.
- 채널은 런타임 소유.

### 9.5 자식

- keyed 트리.
- 안 쓰는 키는 unmount (상태 초기화).
- 같은 키는 재사용 (상태 보존).

---

## 10. 매크로 전개

### 10.1 `#[view]`

```rust
// 입력
#[view]
fn Counter(n = 0) {
    <Col>"Count: {n}"</Col>
}

// 전개 (개념)
pub struct Counter;
impl Component for Counter {
    type Props = CounterProps;
    fn render(props: &Props, ctx: &Ctx) -> Element {
        let n = ctx.slot(0, || props.n);
        Col { children: vec![
            Text { value: format!("Count: {}", n) }
        ]}
    }
}
```

### 10.2 `ui!`

- `<Tag attr=value>` → `Tag { attr: value }`
- `{expr}` → `expr.into_elements()`
- `"text {n}"` → `text(format!("text {}", n))`
- `{n += 1}` (이벤트 안) → `ctx.mutate(0, |v| *v += 1)`

### 10.3 `css!`

- `.class { prop: value }` → `Style { ... }`
- `.class` → `StyleId(u32)`
- 렌더 시 `HashMap<StyleId, Style>` 조회.

### 10.4 `store!`

- 필드마다 슬롯 생성.
- `app.field` → `ctx.store::<T>(TypeId, field_index)`

---

## 11. 파일 구조

```
elm-magic/
├── Cargo.toml
├── src/
│   ├── lib.rs              // public API
│   ├── prelude.rs          // use elm_magic::prelude::*
│   ├── element.rs          // Element enum
│   ├── state.rs            // State<T>, 아레나
│   ├── runtime.rs          // Cmd, Sub, 스케줄러
│   ├── platform/
│   │   ├── mod.rs          // Platform trait
│   │   ├── egui.rs         // egui 어댑터
│   │   ├── web.rs          // wasm 어댑터
│   │   ├── terminal.rs     // TUI 어댑터
│   │   └── headless.rs     // 테스트 어댑터
│   ├── macros/
│   │   ├── view.rs         // #[view]
│   │   ├── ui.rs           // ui!
│   │   ├── css.rs          // css!
│   │   └── store.rs        // #[store]
│   └── testing.rs          // mount!, assert_*
└── examples/
    ├── counter.rs
    ├── todo.rs
    ├── dashboard.rs
    └── chat.rs
```

---

## 12. 사용자 계약

| 계약 | 강제 수단 |
|---|---|
| 컴포넌트는 순수 함수 | `#[view]`가 부수효과 차단 |
| 같은 상태 → 같은 트리 | `Element`가 값이므로 |
| 자식 상태는 부모에 안 샘 | 타입 시스템 + keyed 트리 |
| 키가 바뀌면 초기화 | unmount 규칙 |
| Cmd는 flush 전까지 실행 안 됨 | 런타임 스케줄러 |
| 뷰는 직렬화 가능 | `Element: Serialize` |
| 상태는 원본을 변형 안 함 | `update`가 순수 함수 |

---

## 13. 트레이드오프

| 항목 | 비용 |
|---|---|
| 컴파일 시간 | 매크로 전개 + 모노모피제이션 |
| 에러 메시지 | `Span` 뭉개짐 (`syn::Error::new_spanned` 필수) |
| IDE 지원 | rust-analyzer가 `ui!` 내부를 부분 지원 |
| 디버깅 | `cargo expand` 필수 |
| 학습 곡선 | "Rust인데 Rust가 아님" |
| escape hatch | `<Raw>` 필요 |
| 생태계 | 초기에는 어휘가 적음 |

---

## 14. 로드맵

### v0.1 — 최소
- `ui!` (Col, Row, Text, Button)
- `#[view]` (지역 상태)
- `mount!` (헤드리스 테스트)

### v0.2 — 효과
- `<-` (async, 스트림)
- `on_mount` / `on_key` / `on_tick`
- `css!` (클래스)

### v0.3 — 플랫폼
- egui 어댑터
- `Platform` trait
- `<Raw>` 탈출구

### v0.4 — 고급
- `#[store]` (전역 상태)
- `<Table virtualized>` / `<VirtualList>`
- `<Dock>` / `<Window>`

### v1.0 — 안정
- 컴파일 시간 최적화
- 에러 메시지 개선
- 웹 / 터미널 어댑터
- 시간여행 디버깅

---

## 15. 예제 — 한눈에

```rust
use elm_magic::prelude::*;

fn main() {
    elm_magic::run(Desktop, App);
}

#[view]
fn App() {
    <Col class="app">
        <h1>"elm-magic demo"</h1>
        <Counter start=0 />
        <TodoList />
        <Servers />
    </Col>
}

#[view]
fn Counter(start = 0) {
    <Col class="card">
        "Count: {start}"
        <Row>
            <Button on_click={start += 1}>"+"</Button>
            <Button on_click={start -= 1}>"-"</Button>
        </Row>
    </Col>
}

#[view]
fn TodoList(items: Vec<Todo> = vec![], text = "") {
    <Col class="card">
        <Row>
            <Input value={text} on_enter={items.push(Todo { text })} />
            <Button on_click={items.push(Todo { text })}>"Add"</Button>
        </Row>
        {items.map(|t| ui! {
            <Row class={if t.done { "done" } else { "" }}>
                <Check checked={t.done} on_change={t.done = !t.done} />
                "{t.text}"
                <Button on_click={items.remove(t)}>"x"</Button>
            </Row>
        })}
        <Text class="muted">"{items.len()} items"</Text>
    </Col>
}

#[view]
fn Servers(servers: Vec<Server> = vec![], status = Idle) {
    <Col class="card">
        <Row>
            <h2>"Servers"</h2>
            <Button disabled={status.loading}
                    on_click={status, servers <- fetch_servers()}>
                "Refresh"
            </Button>
        </Row>
        {match status {
            Loading  => ui! { <Spinner /> },
            Failed(e) => ui! { <Banner kind="error">{e}</Banner> },
            _        => ui! { servers.map(|s| <ServerRow server={s} />) },
        }}
    </Col>
}

css! {
    .app   { gap: 16; padding: 24; }
    .card  { gap: 8; padding: 16; bg: surface; radius: 8; }
    .muted { color: text_dim; }
    .done  { text-decoration: line-through; color: text_dim; }
    button { bg: primary; color: on_primary; padding: 8 16; radius: 6; }
    h1     { font-size: 24; weight: bold; }
    h2     { font-size: 18; weight: bold; }
}
```

---

## 16. 한 줄 요약

> **elm-magic은 Rust의 타입 시스템을 유지하면서, 매크로로 문법을 JS/JSX처럼 위장한 초경량 순수 함수형 UI 라이브러리다. 상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자. 다섯 문법이 전부이고, 나머지는 매크로 뒤에 숨는다.**
