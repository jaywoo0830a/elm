# Changelog

이 프로젝트는 [Semantic Versioning](https://semver.org/)을 따른다.
0.x 동안에는 마이너(0.5 → 0.6)가 기능 확장, 패치(0.5.0 → 0.5.1)가 버그 픽스를 뜻한다.

## [0.7.1] — 2026-09-18

`on_change`의 버그 두 개를 고치고 **gpui-kit 어댑터**(`elm-magic-gpui`)를 추가했다.
공개 API 변경 없음 — 어댑터 크레이트가 하나 늘었을 뿐이다.

### Added — gpui-kit 어댑터 (`elm-magic-gpui`)

`elm-magic-egui`와 **같은 계약**을 gpui 위에서 구현한 새 크레이트.
이번 릴리즈가 첫 퍼블리시다.

- `palette(theme) -> Palette` — elm-magic 토큰 14개를 gpui-kit `ThemeColor`에
  매핑한다. 팔레트는 **활성 gpui-kit 테마(`ActiveTheme`)에서 파생**되므로,
  라이트/다크/커스텀 테마를 바꾸면 `css!` 색도 따라온다 (`bg: surface`가
  테마를 따라간다).
- `apply()` — 코어가 해석한 `ResolvedStyle`을 gpui 스타일로 옮긴다.
  **코어는 계산만, 어댑터는 매핑만** 한다는 egui 어댑터의 분업을 그대로 지킨다.
- `ElmView<C>`가 gpui `Render`를 구현하고 모든 위젯 variant를 그린다.
  `<Raw>`는 `&mut gpui_kit::Window`를 받는다.
- README에 gpui-kit 사용 절을 추가했다.
- 의도된 한계: `:hover` / `:active` / `:focus` 슈도 상태, `Modal` 오버레이,
  Input의 커서·IME·선택은 아직 매핑하지 않는다.

### Fixed — `on_change` 콜백 두 가지 (콜백 계약 테스트가 잡음)

`tests/callbacks.rs`의 코어 콜백 계약(1부)을 고정하는 과정에서 드러난
`on_change`의 버그 두 개 (`crates/elm-magic-macros/src/jsx.rs`):

- **마운트 시 발화**: 첫 렌더에서 `pending = None → 초기값`을 "변경"으로
  오인했다. 그래서 `on_change(x) { … }`가 마운트에서 한 번 실행됐고,
  `on_change(x) after 300ms`는 값이 한 번도 바뀌지 않아도 시계만 흐르면
  발화했다. 이제 첫 렌더의 값을 **기준선**으로 삼고 발화하지 않는다
  (마운트 시점 작업은 `on_mount`가 담당한다).
- **재무장 실패**: 값이 바뀌어 디바운스를 다시 무장할 때 `fired = false`를
  슬롯에 되쓰지 않아, 다음 렌더가 이전의 `true`를 읽었다. 결과적으로
  `on_change(x) after …`가 **컴포넌트 수명당 최대 1회**만 발화했다.
  이제 무장 상태를 매 렌더 되쓴다.
- **회귀 테스트**: `tests/callbacks.rs` — 위 두 버그를 직접 재현하는
  `immediate_on_change_does_not_fire_on_mount`,
  `debounce_does_not_fire_on_mount_even_after_the_delay_elapses`,
  `debounce_restarts_on_every_change_and_delivers_the_latest_value` 포함

### Tests

- `tests/callbacks.rs`에 **1부 — 코어 콜백 계약**(어댑터 무관) 섹션 **31개**를
  통합 — 기존 `callbacks.rs`(콜백 prop + children, 2부)와 한 파일에 모았다.
  어댑터를 거치지 않고 코어 콜백 계약만 검증한다: `on_click` /
  `on_change`(값·bool) / `on_enter` / `on_change … after` / `on_mount` /
  `on_unmount` / `on_key` / `on_tick` / `on_event`+`bus.emit` /
  `on_net_change` / `on_navigate` / `<-` 효과 / 콜백 prop.
  명세가 모호한 지점(같은 키 중복 등록, 놓친 틱, `disabled` 클릭)은
  "현재 의미론" 주석과 함께 고정해, 바뀌면 테스트가 알려준다.
- `cargo test --workspace` = **183** (0.7.0: 152), `--all-features` = **187** (156)

## [0.7.0] — 2026-09-18

**Widget Protocol** — `Element`가 데이터 enum에서 **위젯 구조체들의 enum**으로 바뀌고,
`enum_dispatch`가 `Widget` 트레이트를 각 variant로 정적 디스패치한다.
3개 크레이트에 흩어져 있던 variant 매칭이 프로토콜 하나로 모였다.

### Added — `Widget` 프로토콜 (`src/widget.rs`)

- `#[enum_dispatch(Widget)] enum Element { Text(TextEl), Button(ButtonEl), ... }`.
  각 variant는 **위젯 구조체**를 감싼다 (`TextEl`, `ButtonEl`, `ColEl`, …). 새 위젯을
  추가하면 컴파일러가 빠진 트레이트 메서드를 잡아준다.
- `Widget` 트레이트가 위젯의 능력을 한 곳에서 답한다: `kind` / `tag` / `class` /
  `children` / `texts_into` / `role` / `label` / `is_interactive` / `is_disabled` /
  `on_click` / `on_value_change` / `on_value_enter` / `on_bool_change` / `value` /
  `checked` / `raw_fn` / `dump`.
- `Role` (접근성 어휘): `Group` `Text` `Strong` `Banner` `Button` `Tab`
  `ColumnHeader` `Cell` `TextBox` `Checkbox` `Progress` `Spinner` `Separator`
  `Dialog` `Raw` `None`.

### Added — role 기반 테스트 셀렉터 & 접근성 트리 (`testing.rs`)

- `Selector` 빌더 + `sel!` 매크로:
  `sel!(role Button, "+")` / `sel!(role Button)` / `sel!(tag input)` /
  `sel!(class card)` / `sel!(text "hi")`.
- `TestApp::click_role(role, label)`, `click_sel(&sel)`, `query(&sel)`, `exists(&sel)`.
- `TestApp::a11y_tree()`, `roles()`, `has_role(role)` — role/라벨만 남긴 접근성 덤프.
- 탐색 헬퍼(`find_click`, `find_check`, `input_matches`, `dump`, `raw::invoke`)가
  **variant 매칭 없이** 프로토콜 질의만으로 동작한다.

### Changed — Breaking

- `Element`의 struct-variant가 tuple-variant로 바뀌었다:
  - `Element::Button { text, .. }` → `Element::Button(ButtonEl { text, .. })`
  - 매크로(`ui!` / `view!`)가 생성하는 코드는 자동으로 새 형태다 (사용자 코드 변화 없음).
  - `Element`를 직접 패턴 매칭하던 코드만 수정이 필요하다 (`tests/counter.rs` 참조).
- 의존성: `enum_dispatch = "0.3"` (절차적 매크로, **컴파일타임 전용** — 런타임 비용 0).
- `Element::class/tag/children/texts/subtree_text`는 이제 `Widget`의 메서드다
  (inherent 메서드로도 그대로 호출 가능).
- egui 어댑터의 `is_disabled`가 `Widget::is_disabled` 디스패치로 대체됐다.

### 직렬화 (feature `serde`)

- 위젯 구조체 각각이 `Serialize`를 얻고, `Element`는 그 구조체들의 enum으로
  직렬화된다. JSON 표현은 0.6과 동일(`{"Button": {..}}`) — 스냅샷 테스트 그대로 통과.

### Tests

- `tests/widget_protocol.rs` 신규 **14개**
- `cargo test --workspace` = **152** (0.6.1: 138), `--all-features` = **156** (142)

## [0.6.1] — 2026-09-18

`css!`가 하이픈이 들어간 클래스 셀렉터를 조용히 버리던 버그 픽스.
BEM(`.tabs__item--active`)이 등록되지 않던 문제 (FreeDF 리포트).

### Fixed — `css!` 하이픈 셀렉터가 조용히 미등록 (버그 리포트 4번째)

- **증상**: `.tabs__item--active` / `.my-class`가 `lookup` / `resolve`에서
  `None` — 컴파일 에러도 경고도 없이 "스타일이 안 먹는다"로만 보였다
- **원인**: 매크로 `join_selector()`가 `-` 하나도 단어로 봐서
  `.tabs__item--active`를 `.tabs__item - - active`로 조인했고, 코어
  `Selector::parse`가 이를 거부한 뒤 등록 루틴이 **조용히 건너뛰었다**
- **수정** (`crates/elm-magic-macros/src/css.rs`):
  - `join_selector()`가 `-` 조각을 앞 식별자에 이어 붙인다 →
    `.tabs__item--active`, `.my-class`가 그대로 등록된다
  - `validate_selector()`가 코어와 **같은 문법**으로 마디(`compound`)를
    컴파일타임에 검사한다 — `.a..b` / `#id` 같은 잘못된 셀렉터는 이제
    `proc macro panicked` 대신 명확한 **컴파일 에러**다
- **수정** (`src/style.rs`): `style::register`가 파싱 실패를 조용히 건너뛰지
  않고 `panic`한다 (직접 호출하는 경우의 마지막 안전망)
- **회귀 테스트**: `tests/bug_report.rs` 버그 4 (3개) — 리포트의 최소 재현
  (`.tabs__item`은 등록, `.tabs__item--active`는 미등록) + `resolve` 매칭 +
  하이픈 클래스 일반형

### Tests

- `cargo test --workspace` = **138** (0.6.0: 135), `--all-features` = **142** (139)
- `tests/bug_report.rs` 6 → 9

## [0.6.0] — 2026-09-17

`css!` no longer merely registers rules — styles actually resolve and render.
Selector matching, cascade, inheritance, and **36 properties across 14 tags**.

### Added — CSS behaviour (spec 6.1–6.3)

- **Five selector forms + states**: `button` (tag), `.card` (class), `*` (universal),
  `.a.b` (compound), `.card Button` (descendant), `Col > Row` (direct child),
  `.a, .b` (selector list), `button:hover` (state)
- **String selectors**: `".card .muted" { ... }` keeps whitespace verbatim, so real CSS
  descendant chains work. Rust tokens do not preserve spaces, so a class-to-class
  descendant must use this form (`.card.muted` means *compound*)
- **Cascade = specificity → declaration order**, like CSS. The order of names inside
  `class="b a"` no longer matters — the rule declared later wins
- **Inheritance**: `color` `font-size` `weight` `font-style` `font-family` `line-height`
  `letter-spacing` `text-decoration` `text-align` `text-transform`
- **States**: `:hover` `:active` `:focus` `:disabled`. The adapter remembers each node's
  rect and focus in egui memory and resolves state on the next frame — the same
  mechanism egui itself uses for interaction, so there is no visible lag
- Public types `Selector` / `Part` / `Node` / `State` / `StateMask` / `Comb`, plus
  `Element::resolved_style_in(ancestors, state, inherited, palette)`

### Added — 9 → 36 properties

- Layout: `gap` `row-gap` `column-gap` `padding` `margin` `width` `height` `min-width`
  `min-height` `max-width` `max-height` `align` `justify` `wrap` `display` `visibility`
- Paint: `bg` `fill` `border-width` `border-color` `radius` `shadow` `shadow-color` `opacity`
- Text: `color` `font-size` `line-height` `letter-spacing` `weight` `font-style`
  `font-family` `text-decoration` `text-align` `text-transform` `truncate`
- Interaction: `cursor`
- New value types: `Len` (`240` / `fill` / `auto`), `Align`, `Transform`, `Cursor`, `Shadow`
- `display: none` renders nothing at all; `visibility: hidden` keeps the space

### Added — palette 8 → 14 tokens, styled tags 6 → 14

- Tokens added: `surface_alt` `success` `info` `border` `shadow` `overlay`
  (alpha support via `Color::rgba`)
- Tags added: `Tab` `Th` `Td` `Check` `Spinner` `Divider` `Progress` `Modal`
  (on top of `Col` `Row` `Text` `Strong` `Button` `Banner`)

### Added — style contract types (spec 6.1)

- Two-stage `StyleSpec` (declaration) / `ResolvedStyle` (resolved). Resolution is
  platform-independent, so most of the style contract is covered by headless tests
- `Palette`, `Color`, `Edges` (1/2/4-value shorthand), `Element::tag()`, `resolved_style()`
- `style::lookup` / `lookup_class` / `lookup_tag` / `resolve` / `resolve_nodes` / `selectors`
- `IntoClasses` now also covers `Option<T>` and `&T`

### Fixed — `css!` consistency

- Class naming unified: `.card` ≡ `card`; tag and class namespaces no longer collide
  (`button { ... }` vs `.button { ... }`)
- `class="a b"` used to match nothing; it is now split on whitespace
- No more silent drops: unknown properties/values, malformed selectors, and duplicates
  inside one block are compile errors
- Tag selectors are case-insensitive (`Button` ≡ `button`)

### Tests

- `tests/style.rs` new, 25 — selector parsing, specificity, descendant/child, states,
  inheritance, all 36 properties, token vocabulary
- `tests/css.rs` 3 → 13 — registration rules, multi/conditional classes, five selector forms
- egui `tests/adapter.rs` 3 → 7 — `Pass::styles`, `display:none`, theme palette
- `cargo test --workspace` = **135**, `--all-features` = **139**
- Adapter tests only inspect our own types (`Pass::styles`), so they survive platform
  version bumps

### Known limitations

- `class` on `Input` / `TextArea` / `Raw` is still ignored
- No inline styles (`style="..."`), no `transition`, no `:not()` / `:nth-child()`
- `css!` registers through an `.init_array` constructor — wasm needs another mechanism

## [0.5.0] — 2026-09-17

사양서의 v0.5 마일스톤(전역 상태 · keyed 트리 · 구독 · 콜백 prop)에
버그 리포트 3건 수정과 직렬화 계약을 더한 첫 배포.

### Added — 전역 상태 & keyed 트리 (사양서 4.2, 9.5)

- `#[store] struct App { … }` / `store_fn! { … }` — 이름 기반 아레나 슬롯 공유 (`app.count += 1`)
- store 필드별 버전 카운터 (`Arena::store_version`)
- 인스턴스 경로 기반 keyed 슬롯 (`Arena::keyed_slot` / `drop_instance`):
  `<Row key={id}>` 재정렬 시 상태 보존, unmount 시 슬롯 초기화
- `elm_magic::frame::<C>(ctx, props)` — 렌더 → unmount/구독 전달 → 재렌더 루프

### Added — 구독 계열 (사양서 5.3, 5.4)

- `on_message(소스, 상태) { … }` (+ `mock_stream!` 스트림 목)
- `on_event(Name) { … }` + `bus.emit(Name)`
- `on_net_change { … }` + `net.is_online()`
- `on_navigate(|path| …)` — `String` 경로 또는 `navigate_value` 라우트 타입
- `on_unmount { … }`

### Added — 콜백 prop & 컴포넌트 children (사양서 3.1)

- `fn ItemRow(item: Item, on_select: fn(Id))` → prop `Option<Callback<Id>>`
- 호출부 `on_select={selected = _}`, 본문 `on_select(item.id)`
- 컴포넌트 children(`props.children` → `{children}`), 클릭 가능한 `<Row>` / `<Col>`
- 모든 prop이 `Option<T>` — 필수 prop은 명확한 panic, `mount_with!`는 `..Default::default()`

### Added — 직렬화 / 스냅샷 (사양서 8.3, **선택 기능**)

- `--features serde`: `Element` / `StyleProps`가 `Serialize`.
  핸들러(`Rc<dyn Fn>`)와 `<Raw>` 클로저는 `#[serde(skip)]` → **구조·텍스트·클래스만** 직렬화
- 기본 빌드는 여전히 **외부 의존성 0** (`cargo tree -p elm-magic -e normal` → 매크로 크레이트뿐)

### Fixed — 버그 리포트 3건 (재현 테스트 먼저)

- **`pub fn` 컴포넌트**: `vis`가 `#[derive(..)]` 앞에도 찍혀 `pub #[derive(..)]`가 되던 문제.
  이제 `pub(crate)` / `pub(super)` / `pub(in path)` 그룹까지 보존 (`view.rs`, `store.rs`)
- **`remove(x)` 뒤 문장 구분자**: `retain` 특수 폼 전개가 종료자(`;`/`,`)를 소비하지 않아
  `,`가 남던 문제 → `expected expression, found ','` 해결
- **지역 컬렉션 `E0716`**: `{if ..}` 식 안 지역 변수의 `.iter().map(..)`이 소유 반복으로
  전개되지 않던 문제 → `(x).clone().into_iter().map(..)` (`jsx.rs::local_iter_sugar`)

### Tests

- `cargo test` = **96개**, `cargo test --workspace --all-features` = **100개**
- 신규: `tests/bug_report.rs` (6), `tests/snapshot.rs` (4)

### Known limitations

- 리스트 아이템 필드 대입(`t.done = !t.done`), 한 행에 아이템 캡처 핸들러 2개 이상 — 미구현
- 어휘 2차 확장(`Table` / `VirtualList` / `Scroll` / `Plot` / `Dock` / `Window` / `Sidebar` /
  `Menu` / `Fade`, 소문자·HTML 태그), 서비스 객체(`ws` / `api`), `#[view]` 속성 매크로 — 미구현
- `Desktop` / `Web` / `Terminal` 플랫폼 — 미구현 (현재 `Headless` + egui 어댑터)
- 상세: `prototype/prototypes/implementation-status.md`

[0.7.1]: https://github.com/jaywoo0830a/elm/releases/tag/v0.7.1
[0.7.0]: https://github.com/jaywoo0830a/elm/releases/tag/v0.7.0
[0.6.1]: https://github.com/jaywoo0830a/elm/releases/tag/v0.6.1
[0.5.0]: https://github.com/jaywoo0830a/elm/releases/tag/v0.5.0
