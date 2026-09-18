# elm-magic

**"상태는 변수, 이벤트는 대입, 화면은 함수 본문, 효과는 `<-`, 플랫폼은 인자."**

Rust 타입 시스템은 그대로 두고, 매크로로 문법을 JS/JSX처럼 위장한 초경량 순수 함수형
UI 라이브러리. 기본 빌드의 외부 의존성은 **0**이다
(`cargo tree -p elm-magic -e normal` → 매크로 크레이트뿐).

## 설치

```toml
[dependencies]
elm-magic = "0.6"
elm-magic-egui = "0.6"   # egui 어댑터 (선택)
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

## 기능 요약

- **상태**: 매개변수 = 상태 슬롯, `n += 1` / `x = y` / `items.push(..)` / `remove(..)`,
  `#[store]` 전역 상태, keyed 슬롯(`key={id}`)
- **효과**: `a, b <- f(..)`, 스트림 `expr -> slot { .. }`,
  `on_mount` / `on_key("Ctrl+S")` / `on_tick(16ms)` / `on_change(x) after 300ms`
- **구독**: `on_message` / `on_event` / `on_net_change` / `on_navigate` / `on_unmount`
- **컴포넌트**: 콜백 prop(`on_select={..}`), children, `mount_with!`
- **테스트**: `flush` / `advance` / `pump` / `click` / `type_` / `press_key` / `render_tree`
- **선택 기능**: `--features serde` → `Element: Serialize` (뷰 스냅샷)

## 문서

- `CHANGELOG.md` — 버전별 변경
- `prototype/prototypes/implementation-status.md` — 구현 현황과 미구현 목록
- `RELEASING.md` — 배포 절차
- API: <https://docs.rs/elm-magic>

## 라이선스

MIT
