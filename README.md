# elm-magic

**"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**

Rust 타입 시스템은 그대로 두고, 매크로로 문법을 JS/JSX처럼 위장한 초경량 순수 함수형
UI 라이브러리. **런타임 의존성은 0**이다 — `cargo tree -p elm-magic -e normal`은
매크로 크레이트와 컴파일타임 전용 `enum_dispatch`뿐이고, 실행 바이너리에는 남지 않는다.

## 설치

```toml
[dependencies]
elm-magic = "0.7"
elm-magic-egui = "0.7"   # egui 어댑터 (선택)
elm-magic-gpui = "0.7"   # gpui-kit 어댑터 (선택)
```

## 빠른 시작

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

#[test]
fn counter_increments() {
    let mut app = elm_magic::mount!(Counter); // 헤드리스 — 렌더러·런타임 불필요
    app.click("+");
    app.expect_text("Count: 1");
}
```

## 스타일 — `css!`

```rust
elm_magic::css! {
    .tabs__item { color: text_dim; }
    .tabs__item--active { color: text; } // BEM 수정자도 그대로 등록된다 (0.6.1)
}
```

셀렉터 5형태(`button` / `.card` / `*` / `.a.b` / `.card Button` / `Col > Row` / `.a, .b`),
상태(`:hover` `:active` `:focus` `:disabled`), 캐스케이드(명시도 → 선언 순서), 상속,
36개 속성 · 14개 태그 · 14개 팔레트 토큰. 잘못된 셀렉터/속성/값은 **컴파일 에러**다.

## egui에 그리기

```rust
use elm_magic::prelude::*;

fn counter_ui(ui: &mut egui::Ui, ctx: &mut Ctx) {
    let tree = elm_magic::frame::<Counter>(ctx, &CounterProps::default());
    elm_magic_egui::render(ui, &tree, &mut ctx.arena);
}
```

## gpui-kit에 그리기

```rust
use elm_magic::prelude::*;
use elm_magic_gpui::ElmView;

struct Hello;
impl Render for Hello {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(cx.new(|cx| ElmView::<Counter>::new(cx)))
    }
}
```

`css!`의 팔레트는 활성 gpui-kit 테마(`ActiveTheme`)에서 온다 — `bg: surface`는
`theme.popover`, `color: text_dim`은 `theme.muted_foreground`로 매핑되므로 라이트/다크/
커스텀 테마를 바꾸면 `css!` 색도 따라온다. `<Raw>`는 `&mut gpui_kit::Window`를 받는다.

## 기능 요약

- **상태**: 매개변수 = 상태 슬롯, `n += 1` / `x = y` / `items.push(..)` / `remove(..)`,
  `#[store]` 전역 상태, keyed 슬롯(`key={id}`)
- **효과**: `a, b <- f(..)`, 스트림 `expr -> slot { .. }`,
  `on_mount` / `on_key("Ctrl+S")` / `on_tick(16ms)` / `on_change(x) after 300ms`
- **구독**: `on_message` / `on_event` / `on_net_change` / `on_navigate` / `on_unmount`
- **컴포넌트**: 콜백 prop(`on_select={..}`), children, `mount_with!`
- **테스트**: `flush` / `advance` / `pump` / `click` / `type_` / `press_key` / `render_tree`
- **품질 테스트**: 사양서 계약·결정성·격리·경계값(`tests/contract.rs` 등),
  `insta` 스냅샷, `trybuild` 컴파일 실패 계약, `proptest` 모델 기반,
  egui·gpui 어댑터 헤드리스 테스트 — `cargo test --workspace` **290개**(`--all-features` 297)
- **CI**: `.github/workflows/ci.yml` — 테스트 3모드 + 테스트 코드 fmt/clippy +
  스냅샷 승인 + 런타임 의존성 0 검사 + `cargo package` ×4 (Windows/macOS·MSRV 포함)
- **위젯 프로토콜 (0.7)**: `Widget` 트레이트 + `enum_dispatch` — `role` / `label` /
  `is_interactive` / `is_disabled` / 핸들러 접근을 variant 매칭 없이
- **셀렉터 (0.7)**: `click_role` / `sel!` / `query` / `exists` / `a11y_tree`
- **선택 기능**: `--features serde` → `Element: Serialize` (뷰 스냅샷)

## 위젯 프로토콜 (0.7)

`Element`는 이제 **위젯 구조체들의 enum**이고, `enum_dispatch`가 `Widget` 트레이트의
메서드를 각 변형(`TextEl`, `ButtonEl`, `ColEl`, …)으로 정적 디스패치한다.
소비 측은 variant를 몰라도 된다 — 스크린리더·테스트·어댑터가 같은 질문을 던진다.

```rust
use elm_magic::prelude::*;
use elm_magic::sel;

elm_magic::view! {
    fn Counter(n = 0) {
        <Row>
            <Button on_click={n -= 1}>"-"</Button>
            "Count: {n}"
            <Button on_click={n += 1}>"+"</Button>
        </Row>
    }
}

#[test]
fn role_selectors_and_a11y() {
    let mut app = elm_magic::mount!(Counter);

    app.click_role(Role::Button, "+");        // role + 라벨로 클릭
    app.click_sel(&sel!(role Button, "-"));   // 셀렉터 슈가
    app.expect_text("Count: 0");

    assert!(app.exists(&sel!(tag button)));   // 태그로 찾기
    assert!(app.has_role(Role::Group));       // Row는 group

    println!("{}", app.a11y_tree());          // group / button "…"
}
```

트리 순회·접근성·이벤트 탐색이 모두 `Widget`의 메서드(`children`, `role`, `on_click`,
`on_value_change`, `on_bool_change`, `raw_fn`, `dump` …)로 통일돼, 새 위젯을 추가할 때
6곳의 `match` 대신 구현 하나만 채우면 된다.

## 문서

- `CHANGELOG.md` — 버전별 변경
- `prototype/prototypes/implementation-status.md` — 구현 현황과 미구현 목록
- `RELEASING.md` — 배포 절차
- API: <https://docs.rs/elm-magic>

## 라이선스

MIT
