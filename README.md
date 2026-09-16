# elm-magic

> **"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**

Rust의 타입 시스템을 유지하면서, 매크로로 문법을 JS/JSX처럼 위장한
초경량 순수 함수형 UI 라이브러리. (사양서: `prototype/prototypes/spec.md`)

## 설치

```toml
[dependencies]
elm-magic = "0.1"
```

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
| **태그** = 뷰 | `<Col>`, `<Row>`, `<Text>`, `<Button>`, `<Input>` | ✅ v0.1 |
| **`<-`** = 효과 | `status, users <- fetch()` — 런타임이 스폰·폴링 | ✅ v0.2 |
| **`mock()`** = 테스트 | `mock!(app, f, |q: T| ...)` · `flush()` · `advance(ms)` · `pump()` · `press_key()` | ✅ v0.3 |

라이프사이클(v0.2): `on_mount { ... }`, `on_key("Ctrl+S") { ... }`,
`on_tick(ms) { ... }`, `on_change(x) after ms { ... }`(디바운스). 스타일: `css!`.

플랫폼(v0.3): `elm_magic::run(Headless, App)` — 플랫폼은 인자. egui 어댑터는
별도 크레이트 `elm-magic-egui`(`render(ui, &tree, &mut arena)`).

## 구조

```
src/
├── lib.rs        # public API (Component, mount!, prelude)
├── element.rs    # Element enum + IntoElements (조건부/이터레이터 통일)
├── state.rs      # Arena 슬롯, State<T> Copy 핸들, Ctx
└── testing.rs    # 헤드리스 TestApp (click / type_ / press_enter / expect_text)
crates/elm-magic-macros/
├── src/jsx.rs    # JSX 토큰 변환기 (태그 → Element 생성 코드)
├── src/view.rs   # view! — 매개변수 → 슬롯, 본문 → ui 변환
└── src/lib.rs    # 매크로 진입점
```

## 테스트

```sh
cargo test
```

- 렌더러도 런타임도 없이 순수 함수 호출만으로 UI 로직을 검증한다 (사양서 8.1).
- `render_tree()`는 Element 트리의 텍스트 덤프 — 스냅샷 계약 (사양서 8.3).

## 매핑 (사양서 10)

| 사용자 문법 | 전개 |
|---|---|
| `fn Counter(n = 0)` | `CounterProps { n: i32 }` + `ctx.slot(0, \|\| props.n.clone())` |
| `on_click={n += 1}` | `Rc::new(move \|_elm_a\| { state.mutate(arena, \|v\| *v += 1) })` |
| `"Count: {n}"` | `Element::Text { text: format!("Count: {}", n) }` |
| `{items.map(\|t\| <Row>...)}` | `into_elements(items.iter().map(...))` |
| `{if x { A } else { B }}` | `into_elements(if ... )` — `IntoElements`로 통일 |
| `<Counter start=0 />` | 인라인 렌더 + 슬롯 베이스 오프셋 (`SLOTS`) |

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
  (`|q: String| ...`) — 다운캐스트 키로 사용. 사양서의 `.mock(fn, impl)`
  메서드 체인 대신 자유 매크로 `mock!(app, fn, |args| ...)`로 구현.
- 효과 목은 `<-`의 단순 호출 `f(args)` 형태만 지원(메서드 호출·복합식은 미지원),
  스트림(`->`) 목은 미지원. 0-인자 효과 함수는 목 불가.
- `mock`은 스레드 로컬 — 각 테스트는 독립 스레드에서 격리됨.
- `css!` 자기등록은 `.init_array` ctor — Linux/macOS 동작, wasm은 v1.0 과제.
- `on_message` / `on_event` / `on_navigate` 구독과 `#[store]`는 v0.4 로드맵.
- egui 어댑터는 Col/Row/Text/Button/Input/Raw만 매핑(`<Scroll>`, `<Plot>` 등은 v1.0).

