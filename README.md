# elm-magic

> **"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**

Rust의 타입 시스템을 유지하면서, 매크로로 문법을 JS/JSX처럼 위장한
초경량 순수 함수형 UI 라이브러리. (사양서: `prototype/prototypes/spec.md`)
(구현 현황 · 미구현 목록: `prototype/prototypes/implementation-status.md`)

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
| `{items.map(\|t\| <Row>...)}` | `into_elements((state.clone()).into_iter().map(...))` — 소유 반복 |
| `{if x { A } else { B }}` | 분기마다 `into_elements(..)` → `Vec<Element>` 통일 (본문이면 `into_element` → `Fragment`) |
| `<Counter start=0 />` | 인라인 렌더 + 슬롯 베이스 오프셋 (`SLOTS`) |

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
- `{_}`는 값 이벤트(`on_change`/`on_enter`)에서만 — `on_click` 등에는 전달값이 없다.
- `state.remove(비정수)`는 `Vec::retain` + `PartialEq`로 전개된다(`items.remove(t)` = 값으로 삭제).
- `items.iter()` / `items.map(…)`는 소유 반복(`into_iter`)이므로 아이템을 수정하려면
  `items` 쪽 메서드를 쓰거나 다시 `push`해야 한다 (`&mut` 이터레이션은 사양서 대상 아님).
- 콜백 prop(`on_select: fn(Id)`)과 **사용자 컴포넌트의 children**은 아직 미구현
  (children은 `<Modal>`처럼 빌트인만).
- `Table`, `VirtualList`, `Scroll`, `Plot`, `Dock`, `Window`, `Sidebar` 등 어휘와
  `Desktop` / `Web` / `Terminal` 플랫폼은 v0.5+ 과제 (egui 어댑터는 현재 17개 태그 매핑).
- keyed 트리(`<Row key={…}>`)는 여전히 무시된다 (렌더 순서 기반 슬롯 오프셋).
- **리스트 아이템 필드 대입은 아직 안 된다**: `items.map(|t| …)`의 `t`는 값 복사본이므로
  `on_change={t.done = !t.done}` 같은 대입은 `E0594`가 난다. 대신 `items` 쪽 메서드를 쓴다
  (예: `<Check checked={t.done} on_change={items.toggle(&t)}>` — 아이템 편집/인덱스 추적은 다음 과제).
- 한 행에서 **같은 아이템을 캡처하는 핸들러가 2개 이상**이면 `move` 캡처가 충돌한다.
  핸들러는 아이템당 하나만 두고, 나머지는 읽기(텍스트/`checked`)로 표현한다.

디버깅: `ELM_MAGIC_DUMP=1 cargo build`로 `view!` 전개 코드를 그대로 볼 수 있다
(사양서 13장의 "`cargo expand` 필수" 항목 대체).

전체 현황: `prototype/prototypes/implementation-status.md`

