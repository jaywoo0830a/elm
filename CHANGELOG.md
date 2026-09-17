# Changelog

이 프로젝트는 [Semantic Versioning](https://semver.org/)을 따른다.
0.x 동안에는 마이너(0.5 → 0.6)가 기능 확장, 패치(0.5.0 → 0.5.1)가 버그 픽스를 뜻한다.

## [Unreleased] — v0.6 스타일

`css!`가 **등록만 하고 렌더링되지 않던** 상태를 끝낸다. egui 어댑터가 스타일을 실제로 적용한다.

### Added — 타입 있는 스타일 (사양서 6.1)

- `StyleSpec`(선언: 팔레트 해석 전) / `ResolvedStyle`(확정: 색까지 해석) 2단 구조.
  해석이 egui를 모르므로 **스타일 계약 대부분이 헤드리스 테스트**로 검증된다
- `Palette` + `Token` 8종(`primary` `on_primary` `surface` `background` `text` `text_dim` `error` `warn`),
  `Color`, `Edges`(1·2·4값 숏핸드), `Element::tag()`, `Element::resolved_style(&Palette)`
- `style::lookup_class` / `lookup_tag` / `resolve` — 캐스케이드는 **태그 < 클래스(나열 순서, 뒤가 이김)**

### Added — egui 적용 (6개 태그, 9개 속성)

- 태그: `Col` `Row` `Text` `Strong` `Button` `Banner`
- 속성: `gap` `padding` `margin` `bg` `color` `radius` `font-size` `weight` `text-decoration`
- `render_with_palette(ui, tree, arena, &theme)` — 기본은 egui 밝기에 따라 `Palette::dark()/light()`
- `Pass::styles` — 스타일이 적용된 엘리먼트를 기록해 어댑터 테스트가 검증할 수 있다
- `<Banner>` 기본 색이 하드코딩 RGB에서 **팔레트 토큰**(`error`/`warn`/`primary`)으로 바뀌어 테마를 따른다

### Added — `class` 속성 정합성

- `IntoClasses` 트레이트: `class={문자열}` / `{String}` / `{Vec<String>}` / `{Vec<&str>}` / `{[&str; N]}`
  → 사양서 6.2의 `class={if error { "error" } else { "ok" }}`가 이제 컴파일된다

### Fixed — `css!` 정합성 (테스트 우선)

- **클래스 규약 통일**: `css!`는 `.card`(점 포함)로 등록하는데 `Element::class`와 테스트 셀렉터는 `"card"`를
  썼다 → `lookup(".card")` ≡ `lookup("card")`. 태그/클래스는 **키 공간을 분리**해
  `button { … }`(태그)와 `.button { … }`(클래스)가 서로를 덮지 않는다
- **`class="a b"`가 매칭 불가**였다: `vec!["a b"]` 하나로 들어가 어떤 셀렉터와도 안 맞던 것을 공백 분리
- **조용한 무시 제거**: 모르는 속성, `:` 없는 선언, 값 종류 불일치, 한 블록 안의 중복 셀렉터/속성이
  모두 **컴파일 에러**가 되었다 (지원 목록을 메시지에 보여준다)
- 태그 셀렉터는 대소문자를 무시한다 (`Button` ≡ `button`)

### Tests

- `tests/style.rs` 신규 (16) — 캐스케이드·팔레트·숏핸드·태그·`IntoClasses`
- `tests/css.rs` 3 → 12 — 규약 통일·다중 클래스·조건부 클래스·**토큰 어휘 대조**
- `crates/elm-magic-egui/tests/adapter.rs` 3 → 8 — 페인트 명령에 배경색이 실제로 칠해지는지까지 확인
- `cargo test` = **126개**, `cargo test --workspace --all-features` = **130개**

### Known limitations

- 스타일 적용 태그 6개 / 속성 9개만 (`Tab` `Th` `Td` `Input` 등은 아직 무시)
- `:hover` / `:focus` / `:disabled` 상태 셀렉터, 자식·후손 셀렉터, CSS 상속·스펙티시티 없음
- `css!` 등록은 `.init_array` ctor — wasm은 다른 메커니즘 필요

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
