# Changelog

이 프로젝트는 [Semantic Versioning](https://semver.org/)을 따른다.
0.x 동안에는 마이너(0.5 → 0.6)가 기능 확장, 패치(0.5.0 → 0.5.1)가 버그 픽스를 뜻한다.

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

[0.5.0]: https://github.com/jaywoo0830a/elm/releases/tag/v0.5.0
