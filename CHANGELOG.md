# Changelog

이 프로젝트는 [Semantic Versioning](https://semver.org/)을 따른다.
0.x 동안에는 마이너(0.5 → 0.6)가 기능 확장, 패치(0.5.0 → 0.5.1)가 버그 픽스를 뜻한다.

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
