# elm-magic — 구현 현황 정리

> 기준: `spec.md`(사양서) + `1.rs` / `2.rs` / `3.rs`(각 20 패턴)
> 대상 구현: `src/`, `crates/elm-magic-macros/`, `crates/elm-magic-egui/`
> 최초 작성: 테스트 36개 → v0.4: 64개 → **v0.5.0: 96개(기본) / 100개(`--all-features`)**

---

## 0. 한 줄 요약

**v0.1~v0.3 스코프(변수 · 대입 · 태그 · `<-` · `mock` + `css!` / 라이프사이클 / 스트림 / 플랫폼)만 동작한다.**
그 밖의 것 — **태그 어휘 확장, 전역 상태(`#[store]`), 구독(`on_message` / `on_event` / `on_navigate`), keyed 트리·가상화, 테스트 슈가, egui 어휘** — 은 대부분 미구현이다.

특히 `1.rs`는 20패턴 중 **절반가량이 컴파일 자체가 되지 않는다.** 문서(`spec.md`, `README.md`)에 적힌 "v0.x 제한" 외에, **문서화되지 않은 매크로 버그 16종**이 추가로 존재한다(3.1절).

---

## 1. 검증 방법

프로토타입의 패턴을 그대로 잘라내어 실제 크레이트에 대해 `rustc`로 컴파일했다.
(리포지토리는 수정하지 않았고, 스니펫은 `/tmp/elmchk/*.rs`에 있다.)

```sh
cargo build
rustc --edition 2021 --test --emit=metadata \
  -L dependency=target/debug/deps \
  --extern elm_magic=target/debug/deps/libelm_magic-*.rlib \
  /tmp/elmchk/<snippet>.rs
```

- `--test`를 붙여야 `#[test]` 함수 본문까지 타입 체크된다(붙이지 않으면 `cfg(test)` 게이트로 건너뛴다).
- 표기 규칙: **✅ 검증됨**(컴파일 또는 테스트 통과) / **❌ 검증됨**(컴파일 실패, 에러 메시지 첨부) / **⚠️ 추정**(코드 리딩 기반, 개별 컴파일은 안 함)

---

## 2. 지금 동작하는 것 ✅

| 영역 | 구현 내용 | 근거 |
|---|---|---|
| 매크로 | `view! { fn … }`(함수형), `ui!`, `css!`, `mock!`, `mount!` / `mount_with` | `crates/elm-magic-macros/src/{view,jsx,css}.rs` |
| 상태 | 매개변수 → 슬롯, `n += 1` / `-=`, `x = y`, `_`(on_change / on_enter), `items.push(x)`, `;` 다중문장, `let` 파생값, `items.map(…)`, 중첩 컴포넌트 + props | `tests/counter.rs`, `tests/todo.rs`, `tests/effects.rs` |
| 뷰 | `Col` / `Row` / `Text` / `Button` / `Input` / `Raw`, `class="x"`, `class={if …}`, `disabled={…}`, `{if …}`, `{match …}`(arm 타입이 같을 때) | `jsx.rs:1093-1140` |
| 효과 | `a, b <- f(…)`, 스트림 `expr -> slot { … }` + `pump()`, `on_mount`, `on_key("Ctrl+S")`, `on_tick(정수)`, `on_change(x) after 정수` | `tests/{effects,stream,lifecycle}.rs` |
| 스타일 | `css!` 등록/조회(`style::register` / `style::lookup`) | `src/style.rs`, `tests/css.rs` |
| 테스트 | `flush` / `advance` / `pump` / `press_key` / `press_enter` / `type_` / `click` / `expect_text` / `text` / `render_tree`, `set_mock1..3` | `src/testing.rs` |
| 플랫폼 | `run(Headless, C) -> TestApp`, egui 어댑터 crate(`render(ui, &tree, &mut arena) -> Pass`, 17태그, `<Raw>` 실제 호출) | `src/platform.rs`, `crates/elm-magic-egui/{src,tests}/` |

---

## 2.5 v0.4 — 구현 완료 10건 ✅

우선순위 상위 10개(버그 7 + 기능 3)를 구현했다. `cargo test --workspace` = **90개 통과**,
`tests/{syntax,timers,widgets,sugar}.rs`가 각 항목의 회귀 테스트다.

| # | 항목 | 상태 | 구현 위치 / 사용법 |
|---|---|---|---|
| 1 | **시간 리터럴** `16ms` `300ms` `5min` `2s` | ✅ | `jsx.rs::duration_expr` — `on_tick(500ms)`, `on_change(x) after 300ms`, `<- f() after 1min` |
| 2 | `..` 범위 뒤 상태 읽기 | ✅ | `jsx.rs::is_field_access` — `{(0..n).map(\|i\| …)}` |
| 3 | 이벤트 본문 상태 메서드(0-인자/반환값 무시/조건문 오인) | ✅ | `jsx.rs::Level`(Stmt/Expr) + `stmt_position` — `submit(name.clone(), email.clone())`, `if user.is_empty()`, `items.remove(0)` |
| 4 | 컴마 다중문 | ✅ | `jsx.rs::stmt_end` — `on_click={a = 1, b = 2}` |
| 5 | 구조체 리터럴 축약 | ✅ | `jsx.rs::shorthand_group`(중괄호 유지 + 경로 판정) — `items.push(Note { body })` |
| 6 | **`{if}` / `{match}` 분기 타입 통일** | ✅ | `jsx.rs::{unify_if,unify_match}` + `element::IntoElements`(IntoIterator blanket) + `Element::Fragment` / `IntoElement` — `{if loading {<Spinner/>} else {items.map(…)}}` |
| 7 | **리스트 아이템 이벤트 캡처(E0716)** | ✅ | `.map(…)`/`.iter()` sugar가 **소유 반복**(`into_iter`)으로 전개 — `items.map(\|t\| <Button on_click={sel = t.clone()}>)` |
| 8 | `after` 없는 `on_change` + `<- … after <dur>` | ✅ | `jsx.rs` on_change(선택적 `after`) + `Arena::spawn_after` / `take_due` / `TestApp::advance`(가상 시계), `has_pending_after()` |
| 9 | **빌트인 태그 11종 + children** | ✅ | `Spinner`, `Divider`, `Strong`, `Banner`, `Check`, `TextArea`, `Tab`, `Th`, `Td`, `Progress`, `Modal`(자식 보유) + `Element::Fragment` |
| 10 | 테스트 슈가 | ✅ | `.mock(fn, impl)` / `.mock2` / `.mock3`(함수명 자동 추출), `assert_text` / `assert_visible` / `assert_hidden`, `type_into(selector, value)`, `toggle(label)` / `set_check(label, v)` |

### 새로 생긴 규칙 (문서화 필요)

| 규칙 | 이유 |
|---|---|
| `on_change(x)`의 `x`는 `Clone + PartialEq`여야 한다 | 값이 바뀌었는지 비교해서 본문을 실행한다 (`on_change(state)` 파생/미러 상태) |
| `{_}`는 **값 이벤트**(`on_change`/`on_enter`)에서만 | `on_click` 등에는 전달되는 값이 없다 — 이제 `elm-magic: `_`는 on_change/on_enter …` 라는 명확한 컴파일 에러를 낸다 |
| `state.remove(정수)` = 인덱스 삭제, `state.remove(그 밖)` = **값으로 삭제** | 사양서 3.3의 `items.remove(t)`를 지원하기 위한 구분 (값 삭제는 `PartialEq` 필요) |
| `items.iter()` / `items.map(…)`는 **소유 반복** | `&T`를 `'static` 핸들러에 캡처할 수 없어서 (E0716) 사양서 3.3의 "`&` 생략"을 그대로 구현 |
| 뷰 본문/분기의 값이 `Vec<Element>`면 `Fragment`로 감싼다 | `fn App() { {if ..} }`처럼 본문 전체가 조건부일 때 |

### 여전히 미구현 (v0.5 기준)

`on_message`의 **서비스 객체**(`ws`/`api`), keyed 경로 성능 최적화(문자열 → 해시), `#[view]` 속성 매크로,
소문자/HTML 태그(`<h1>`), `<Table>`/`<VirtualList>`/`<Plot>`/`<Dock>`/`<Window>`/`<Menu>`/`<Fade>` 등 어휘,
`Desktop`/`Web`/`Terminal` 플랫폼, `memo!`, i18n, 클로저 내부 `<-`, `mount(엘리먼트)`,
**리스트 아이템 필드 대입**(`t.done = !t.done`).

### 남은 두 가지 리스트 제약 (다음 마일스톤 1순위)

| 제약 | 증상 | 우회/대안 |
|---|---|---|
| **아이템 필드 대입** `on_change={t.done = !t.done}` | `E0594` (아이템은 값 복사본) | `items` 쪽 메서드를 만든다: `<Check checked={t.done} on_change={items.toggle(&t)}>` — 근본 해결은 인덱스 추적(아이템 → `items[i]` 대입 전개) |
| **한 행에 아이템 캡처 핸들러 2개 이상** | `E0382`/`E0502` (`move` 캡처 충돌) | 핸들러는 아이템당 하나, 나머지는 읽기로 — 근본 해결은 아이템을 `Rc`로 바인딩해 핸들러마다 `Rc::clone` |

디버깅 팁: `ELM_MAGIC_DUMP=1 cargo build`로 `view!` 전개 코드를 덤프할 수 있다
(`crates/elm-magic-macros/src/view.rs`).

### 재측정 (v0.5)

| 항목 | v0.4 | v0.5 |
|---|---|---|
| 워크스페이스 테스트 | 64 | **90** |
| `/tmp/elmchk` 스니펫 통과 | 41 / 65 | **46 / 66** (v0.5 스모크 포함) |
| 테스트 파일 | `syntax`·`timers`·`widgets`·`sugar` | + `store`(6) · `subs`(8) · `callbacks`(6) · `integration`(6) |

---

## 2.6 v0.5 — 구현 완료 3건 ✅ (우선순위 5·6·7)

| # | 항목 | 상태 | 구현 / 사용법 |
|---|---|---|---|
| 5 | **`#[store]` 전역 상태 + keyed 트리** | ✅ | `#[store] struct App { count: i32 }` · `store_fn! { Settings { level: i32 = 3 } }` → `app.count += 1`. 슬롯은 아레나에 **이름 기반**으로 저장 (`Arena::store`/`store_get`/`store_set`/`store_version`)되고 `State<T>`가 `SlotKey::{Index, Store}`로 주소를 갖는다. keyed 슬롯은 인스턴스 경로 `"/<key 또는 @순번>:<컴포넌트>#<슬롯>"` → `Arena::keyed_slot`/`drop_instance`, 프레임 경계(`Ctx::begin_frame`/`end_frame`)에서 사라진 경로의 슬롯을 초기화 |
| 6 | **구독 계열** | ✅ | `on_message(소스, 이름) { … }`(= `->` 스트림 + 인스턴스당 1회 가드) + **스트림 목** `mock_stream!`; `on_event(Name) { … }` + `bus.emit(Name)`; `on_net_change { … }` + `net.is_online()`; `on_navigate(\|v\| …)`(String 경로 또는 `navigate_value` 라우트 타입); `on_unmount { … }` (keyed 트리 연동) |
| 7 | **콜백 prop + 사용자 컴포넌트 children** | ✅ | `fn ItemRow(item: Item, on_select: fn(i32))` → prop `Option<Callback<i32>>`. 호출부 `on_select={selected = _}`는 `Callback::new(move \|_elm_a, _elm_v\| …)`로, 본문 `on_select(item.id)`는 `__elm_cb.call(arena, …)`로 전개. 자식은 `props.children` → 본문의 `{children}`이 렌더. `<Row on_click={…}>` 클릭 가능 컨테이너 추가 |

### v0.5에서 함께 바뀐 것

| 변경 | 이유 |
|---|---|
| **모든 prop이 `Option<T>`** (`None`이면 선언된 기본값, 필수 prop은 명확한 panic) | 필수 prop(`item: Item`)을 기본값 없이 지원. `mount_with!`는 `Some(…)` + `..Default::default()` |
| 프레임 루프 `elm_magic::frame::<C>(ctx, props)` | 렌더 → unmount/구독 전달 → 재렌더를 한 곳에 (테스트·플랫폼 공용) |
| 슬롯 정체성 = **인스턴스 경로** (base 오프셋 → 경로 키) | keyed 트리·unmount·형제 분리. 부수적으로 "키 없는 리스트가 슬롯을 섞던" 기존 버그도 해결 |
| 문자열 보간 `{x}` + `{x:?}` / `{x:.2}` | 사양서 예제의 Debug 출력 |
| `Element::Col/Row`의 `on_click`, `Element::texts()/subtree_text()` | `<Row on_click>` 클릭 (헤드리스 + egui) |
| `elm_magic::clone_value` / `nav_take` | 콜백·라우팅에서 호출부 타입 추론이 되도록 (`_elm_v.clone()`은 추론 실패) |

---

## 2.7 v0.5 — 버그 리포트 3건 ✅ (테스트 우선)

`elm-magic-bug-report.md`(FreeDF 채택 과정)의 3건을 **재현 테스트 먼저**(`tests/bug_report.rs`, 6개)로
고정하고 수정했다. 세 건 모두 리포트의 재현 코드가 그대로 `cargo test`에서 돌아간다.

| # | 증상 | 원인 | 수정 위치 |
|---|---|---|---|
| 1 | `pub fn` / `pub(crate) fn` 컴포넌트가 `visibility pub is not followed by an item` / `macro expansion ignores #` | `vis`가 `#[derive(Clone)]` **앞**에도 찍혀 `pub #[derive(..)]`가 됐고, `pub(crate)`의 괄호 그룹은 파싱에서 버려져 `pub`으로 넓어졌다 | `view.rs::expand`(vis 그룹 보존) + head 생성부(`vis`는 `struct` 앞에만), `store.rs::expand_struct` 동일 |
| 2 | `on_click={items.remove(x), n = 1}` → `expected expression, found ','` | `remove(비정수)`(`retain`) 특수 폼이 문장 종료자(`;`/`,`)를 소비하지 않아 `,`가 그대로 남았다 (대입·일반 메서드는 소비함) | `jsx.rs::transform_event` — 특수 폼 뒤 `;`/`,` 소비 후 `;` 재방출 |
| 3 | `{if ..}` 식 안 지역 컬렉션 `.iter().map(..)` → `E0716` | 슈가는 **상태 슬롯**(`substitute_read`)만 소유 반복으로 전개 — 렌더 본문의 지역 `let` 변수는 건드리지 않았다 | `jsx.rs::local_iter_sugar` — 상태가 아닌 식별자 `x.iter().map(..)` → `(x).clone().into_iter().map(..)` |

### 남은 것 (변동 없음)

- 아이템 필드 대입 `t.done = !t.done`(인덱스 추적), 아이템당 핸들러 2개 이상(`Rc` 바인딩) — 우선순위 8.
- 어휘 2차 확장(우선순위 9) · 서비스 객체/`#[view]`(10) · 플랫폼(11) · 스냅샷의 insta 연동(12, `Element: Serialize`는 2.8절에서 완료).

---

## 2.8 의존성 정책 — `serde`만 선택적, 나머지 3종은 넣지 않음

`serde` / `thiserror` / `enum_dispatch` / `tokio` 도입을 검토한 기록.
**기본 빌드의 외부 의존성은 0**을 유지한다 (`cargo tree -p elm-magic -e normal` → `elm-magic-macros`뿐).

| 라이브러리 | 판단 | 근거 (코드 실측) |
|---|---|---|
| `serde` | ✅ **선택 기능** | 사양서 8.3·부록의 "뷰는 직렬화 가능"(`Element: Serialize`). `[features] serde = ["dep:serde"]` + `#[cfg_attr(feature = "serde", derive(serde::Serialize))]`. 핸들러/`<Raw>`는 `#[serde(skip)]` → **구조·텍스트·클래스만** 직렬화. 기본 그래프는 그대로 0 |
| `thiserror` | ❌ | `src/`+매크로에 `panic!` **54곳**, 공개 API는 panic 기반(매크로 사용 오류를 한국어 메시지로 즉시 실패). 반환형 `Result`/에러 타입이 없어 `#[derive(Error)]`를 넣어도 `panic!("{}", e)` 래핑만 늘어난다 (코드 감소 0) |
| `enum_dispatch` | ❌ | `Element`는 데이터 enum(패턴 매칭: `element.rs::collect_texts`, `testing.rs::dump`, egui `render_el`)이고 핸들러는 **클로저**(고유 타입)라 위임할 트레이트 대상이 없다. variant를 구조체로 바꾸면 3개 크레이트의 매칭을 전부 갈아엎어야 함 — 단순화가 아니라 비용 |
| `tokio` | ❌ | `Arena::spawn` 제약은 `F: Future + 'static`(**`Send` 없음**)이고 테스트는 동기 `flush` 모델. `tokio::spawn`은 `Send`를 요구하므로 `Rc` 캡처 future가 깨진다. `runtime::block_on`의 noop-waker 스핀은 진짜 I/O future에 부적합하지만, 그건 **설계 결정**(headless = 즉시 완료 future)이지 라이브러리 교체 문제가 아니다 |

### `serde` 사용법 (사양서 8.3)

```sh
cargo test -p elm-magic --features serde   # tests/snapshot.rs (4개)
cargo test --workspace --all-features      # 전체 100개
```

```rust
let json = serde_json::to_string(app.element())?;   // 핸들러는 빠진다
```

- `Element` 전 variant의 `on_click` / `on_change` / `on_enter` / `on_close` / `widget`에 `#[serde(skip)]`
- `StyleProps`(style.rs)도 직렬화 가능
- `render_tree()` = 사람이 읽는 텍스트 덤프, `serde` = 기계가 비교하는 스냅샷(insta 등). 둘은 보완 관계

---

## 3. 미구현 목록 ❌

### 3.1 매크로/문법 — 컴파일이 아예 안 되는 것 (전부 검증됨)

> v0.4에서 **1~6, 10, 12, 14는 수정됨**(2.5절 참고). 아래는 당시 진단 기록이며,
> ✅ 표시된 행은 이제 동작한다.

| # | 미구현 항목 | 대표 스니펫 | 당시 에러 | 원인 위치 |
|---|---|---|---|---|
| 1 | **시간 리터럴 `ms` / `min`** | `on_tick(16ms)`, `on_change(q) after 300ms`, `<- f() after 5min` | `invalid suffix 'msu64' for number literal` | ✅ v0.4 — `duration_expr` |
| 2 | `..` 범위 뒤 상태 읽기 | `{(0..n).map(…)` | `cannot find value 'n'` | ✅ v0.4 — `is_field_access` |
| 3 | 이벤트 본문에서 상태 메서드 **0-인자** 호출 | `on_click={submit(name.clone(), email.clone())}`, `if user.is_empty()` | `expected expression, found ';'` | ✅ v0.4 — `Level::Stmt/Expr` |
| 4 | 이벤트 메서드 호출의 반환형 제약 / 요소 삭제 | `on_click={items.remove(t)}`, `items.remove(0)` | `E0308` (Vec::remove는 값 반환·인덱스 기반) | ✅ v0.4 — 반환값 무시 + `retain` sugar |
| 5 | 리스트 아이템을 이벤트에서 캡처 | `items.map(\|t\| <Button on_click={sel = t.clone()}>` | `E0716` temporary dropped while borrowed | ✅ v0.4 — 소유 반복 |
| 6 | 구조체 리터럴 축약 | `items.push(Todo { text })` | `expected ',', found 'text'` | ✅ v0.4 — `shorthand_group` |
| 7 | 컴포넌트 children(사용자 컴포넌트) | `<Item on_click={…}>"New"</Item>`, `<Window>…</Window>` | `proc macro panicked` ("cannot have children") | ❌ **빌트인은 가능**(`<Modal>` 등) — 사용자 컴포넌트는 아직 |
| 8 | 콜백 prop(함수 타입) | `fn ItemRow(item: Item, on_select: fn(Id))` | panic `must have a default value` | `view.rs:166` |
| 9 | 소문자/HTML 태그 | `<h1>`, `<h2>` | panic `unknown tag` | `jsx.rs` |
| 10 | 컴마 다중문 | `on_click={sort = Name, dir = dir.toggle()}` | `E0070` / `E0308` | ✅ v0.4 — `stmt_end` |
| 11 | `{_}`를 클릭류 이벤트에 사용 | `on_click={selected = Some(_)}` | `cannot find value '_elm_v'` | ⚠️ v0.4 — 이제 **명확한 에러**만 (콜백 prop이 생기면 해결) |
| 12 | `after` 없는 `on_change` | `on_change(state) { … }` (시간여행) | panic | ✅ v0.4 (`Clone + PartialEq` 필요) |
| 13 | 클로저 내부 효과 | `let start = \|\| { … <- f() };` | `unexpected token '<-'` | 효과는 문장 위치만 |
| 14 | if/else 분기 타입 혼합 | `{if loading {<Spinner/>} else {users.map(…)}}` | `E0308` (Element vs iterator) | ✅ v0.4 — 분기 통일 |
| 15 | `#[view]` 속성 매크로 | `#[view] fn Counter(n = 0)` | rustc 파싱 단계에서 실패 (`=` 발견) | 비-Rust 문법 → 함수형 `view! { }`만 |
| 16 | 키워드 속성명 | `<Fade in={visible} duration={200}>` | panic | `in`이 키워드 (`<Fade>` 자체도 미구현) |

### 3.2 태그 어휘 — 내장 17개

- **구현:** `Col`, `Row`, `Fragment`(묶음), `Text`, `Strong`, `Button`, `Input`, `TextArea`, `Check`,
  `Tab`, `Th`, `Td`, `Banner`, `Spinner`, `Divider`, `Progress`, `Modal`(children), `Raw`
- **미구현(프로토타입에 등장):**

  | 파일 | 미구현 태그 |
  |---|---|
  | `1.rs` | `Table`, `DropZone`, `Card`, `Fade`, `ErrorBoundary`, `Theme`, `Layout`, `Sidebar`, `Main` — ~~Divider/Strong/Spinner/Banner/Check/Modal/Tab/Th/Td/TextArea~~ ✅ v0.4 |
  | `2.rs` | `Sidebar`, `Center`, `TopBar`, `BottomBar`, `NavMenu`, `MenuBar`, `Menu`, `Item`, `Sep`, `Dock`, `Scroll`, `Collapsing`, `Switch`, `Slider`, `Plot`, `Line`, `Points`, `HLine`, `Candlestick`, `Volume`, `Table(virtualized)`, `Column`, `Image`, `DragValue`, `Gradient`, `FileTree`, `Window` — ~~Progress~~ ✅ v0.4 |
  | `3.rs` | `VirtualList`, `PostCard`, `Skeleton`, `UserCard`, `SplitPane`, `Kbd`, `DevToolbar`, `Status`, `ChatBubble` — ~~Check~~ ✅ v0.4 |

- **속성이 조용히 버려지는 것**(오류 없음 → 더 위험): `<Row key={…}>`, `<Text strike={…}>`, `auto_focus`, `role`, `draggable`, `highlight`, `accept`
  → keyed diffing / a11y / 조건부 스타일 prop 미구현. (`active`/`checked`는 v0.4에서 `<Tab>`/`<Check>` prop으로 구현됨)

### 3.3 상태 모델

- `#[store]` / `app.theme` 전역 상태: **속성 매크로 자체가 없음** → `cannot find attribute 'store'`
- keyed 트리, 키 기반 재사용·unmount·초기화: 없음 (슬롯은 `ctx.base` 렌더 순서 오프셋 — `README.md` v0.1 한계에 명시)
- 사양서 4.4의 `State<T>: Deref / DerefMut` + 아레나 `unsafe` → 실제는 `Any` 다운캐스트 + `get/set/mutate`
- 사양서 9.1의 `Element<Msg>` + `Msg enum` 정적 디스패치 → 실제는 `Rc<dyn Fn>` 핸들러 (**트리 핫패스에 `dyn` 존재**)
- `Element: Serialize` → ✅ **선택 기능**(`--features serde`, 2.8절) · `StyleId(u32)` 인터닝 → 없음 (`Vec<String>` + `HashMap<String, …>`)
- `memo!` 매크로 → 없음
- "조건문 안에서 상태 선언 금지" 같은 컴파일 규칙 검사 → 없음

### 3.4 효과 / 구독

- **구현:** `a, b <- f(…)`(단순 호출만 mock 대상), 스트림 `expr -> slot { … }`, `on_mount`, `on_key`, `on_tick`, `on_change … after`
- **미구현:** `on_unmount`, `on_message(ws, m)`, `on_event(Refresh)`, `on_net_change`, `on_navigate`, `<- … after 5min`(백그라운드 갱신), `rollback <-`(자동 되돌림 **의미론** 없음), `spawn_worker`/스레드 워커, `cache.get(id)`, `net.is_online`, `bus.emit`, `i18n::use_lang`
- **`mock!` 한계**(README v0.3 제한과 일치):
  - `<- f(args)` 단순 호출만 목 가능
  - 메서드 호출 효과(`api.toggle_like(id)`) 목 불가
  - 스트림(`->`) 목 불가
  - 0-인자 효과 함수 목 불가

### 3.5 테스트 API

| 구분 | 항목 |
|---|---|
| ✅ 구현 | `mount!`, `mount_with`, `flush`, `advance(ms)`, `pump`, `press_key`, `press_enter`, `type_(값)`, `click(라벨)`, `expect_text`, `text`, `render_tree`, `set_mock1..3` |
| ✅ v0.4 | `.mock(fn, impl)` / `.mock2` / `.mock3` (소유 빌더), `assert_text` / `assert_visible` / `assert_hidden`, `type_into(셀렉터, 값)` — 셀렉터는 태그명(`input`/`textarea`) 또는 클래스명, `toggle(label)` / `set_check(label, v)`, `has_pending_after()` |
| ❌ 미구현 | 2인자 `type_(셀렉터, 값)`(오버로드 불가 → `type_into` 사용), `advance(300ms)`(리터럴 불가 → `advance(300)`), `Cart(items: vec![…])` 생성자 슈가, `mount(ui! { <Counter /> })`(헤드리스 엘리먼트 마운트), `insta` 스냅샷, 시간여행 |

### 3.6 플랫폼 / 어댑터

- **`Desktop` / `Web` / `Terminal` 플랫폼: 타입 자체가 없음** → `cannot find value 'Desktop'`
- egui:
  - 진입점 `rui::egui("My App", \|\| ui! { … })` 없음 (구현은 `render(ui, &tree, &mut arena) -> Pass`만)
  - 렌더 루프 / 이벤트 펌프 / 상태 변경 시 자동 재렌더 없음 (테스트가 `frame()` + `refresh()`를 직접 호출)
  - 어댑터 매핑은 `Col/Row/Text/Button/Input/Raw` 6개뿐 — 사양서 7.2 표의 `SidePanel`, `TopBottomPanel`, `ScrollArea`, `CollapsingHeader`, `Window`, `menu`, `Plot`, `Table`, `Dock` 전부 없음
- `css!` 자기등록은 `.init_array` ctor → Linux/macOS만, **wasm 미지원**

### 3.7 구조 / 성능 (사양서 11, 9)

| 사양서 | 실제 |
|---|---|
| `src/prelude.rs`, `src/platform/{mod,egui,web,terminal,headless}.rs`, `src/macros/{view,ui,css,store}.rs` | `platform.rs` 단일, `prelude`는 `lib.rs` 인라인 모듈, 매크로는 별도 crate |
| `examples/{counter,todo,dashboard,chat}.rs` | `examples/` 디렉터리 없음 |
| bump arena, `SmallVec`, 문자열 인터닝, async 상태머신 모노모피제이션 | 없음 — `Vec<Option<Box<dyn Any>>>`, `Vec<String>`, `block_on` 스핀 루프 |

---

## 4. 프로토타입 파일별 대응

### 4.1 `1.rs` (기본/일상 패턴 20)

| # | 패턴 | 상태 | 비고 |
|---|---|---|---|
| 1 | 기본 상태(Counter) | ✅ | — |
| 2 | 입력 + 리스트 | ❌ | `items.remove(t)`(값 삭제·반환형), 아이템 캡처 `E0716` |
| 3 | 폼(여러 필드) | ✅ | 단, `submit(name.clone(), email.clone())`처럼 이벤트 인자에 0-인자 메서드를 쓰면 ❌ (`submit(name, email)`는 ✅) |
| 4 | 파생 값(Cart) | ✅ | — |
| 5 | 조건부 렌더(match) | ⚠️ | 각 arm이 동일 타입이면 ✅(`s39`), `<Banner>{e}</Banner>`처럼 children 있는 컴포넌트면 ❌ |
| 6 | 비동기 + 로딩 | ❌ | `<Spinner/>` 없음 + `if/else` 분기 타입 혼합 `E0308` |
| 7 | 자식 컴포넌트 + 콜백 | ❌ | 함수 타입 prop(기본값 없음) panic, children 불가 |
| 8 | 전역 상태(`#[store]`) | ❌ | 매크로 없음 |
| 9 | 효과(on_mount / tick) | ❌ | `tick_every(1000ms)` → `ms` 리터럴. 정수 인자면 ✅ |
| 10 | 디바운스 검색 | ❌ | `after 300ms` → `msu64` (`after 300`이면 ✅, `<Col>` 내부 위치도 ✅) |
| 11 | 페이지네이션 | ✅ | — |
| 12 | 테이블 + 정렬 | ❌ | `<Th>` 태그 없음 + 컴마 다중문 |
| 13 | 탭 | ⚠️ | `tab = Home` 대입은 ✅, `<Tab …>"Home"</Tab>`(children)은 ❌ |
| 14 | 모달 | ❌ | `<Modal>` 없음 + children |
| 15 | 키보드 단축키 | ✅ | `on_key` + `text = undo()` ✅ |
| 16 | 드래그 앤 드롭 | ❌ | `<DropZone>`/`<Card>` 없음, 클릭류 `{_}` ❌ |
| 17 | 애니메이션 | ❌ | `<Fade>` 없음 + `in` 키워드 + `200ms` |
| 18 | 에러 경계 | ❌ | `<ErrorBoundary>` 없음 |
| 19 | 테마 + 플랫폼 | ❌ | `<Theme>` 없음 + `Desktop`/`Web`/`Terminal` 없음 |
| 20 | 테스트(카트 합계 등) | ❌ | `.mock()`, `assert_text`, `advance(300ms)`, `Cart(items: …)` 전부 미구현 |

### 4.2 `2.rs` (egui 통합 20)

| # | 패턴 | 상태 |
|---|---|---|
| 19 | `<Raw>` 탈출구 | ✅ (헤드리스에서 무시, egui 어댑터가 실제 `&mut egui::Ui` 전달) |
| 1 | `rui::egui("My App", \|\| ui! { … })` 진입점 | ❌ |
| 2~18, 20 | Sidebar/Center/TopBar/BottomBar/Panels/Dock/Scroll/Collapsing/Plot/Line/Points/HLine/Candlestick/Volume/Table(virtualized)/Column/Image/DragValue/Gradient/FileTree/`menu!`/`tooltip!`/rfd/Loader/MenuBar/Menu/Item/Sep/Window | ❌ |

### 4.3 `3.rs` (고급 패턴 20)

| # | 패턴 | 상태 | 비고 |
|---|---|---|---|
| 1 | Undo / Redo | ⚠️ | `on_key` + `history[cursor]` 읽기 ✅, `history.push(text.clone())` ✅(인자 위치는 OK), 그 외 세부 제약 있음 |
| 2 | 낙관적 업데이트 | ⚠️ | `liked = !liked;` ✅, `revert <- …`는 파싱되나 **자동 롤백 의미론 없음** |
| 3 | stale-while-revalidate | ❌ | `<- … after 5min`, `cache.get(id)` 기본값 |
| 4 | WebSocket 구독 | ❌ | `on_message`, ws |
| 5 | 가상 스크롤 | ❌ | `<VirtualList>` |
| 6 | DnD 칸반 | ❌ | `<DropZone>`/`<Card>`/`accept`/`highlight` |
| 7 | 커맨드 팔레트 | ⚠️ | `palette_open = false; cmd.run()` ✅, `<Modal>`/`auto_focus`/`<Kbd>` ❌ |
| 8 | 폼 검증 | ✅ | 파생 값 `let …` + `disabled={…}` |
| 9 | Wizard | ✅ | `step` + `match` |
| 10 | 라우팅 | ❌ | `on_navigate` 없음 (route enum + match 자체는 ✅) |
| 11 | i18n | ❌ | `i18n::use_lang()` |
| 12 | A11y | ❌ | `<FocusTrap>`, `role`, `auto_focus` |
| 13 | 키보드 네비게이션 | ✅ | `on_key` + `cursor` |
| 14 | 이벤트 버스 | ❌ | `bus.emit` / `on_event` |
| 15 | 업로드 스트림 | ✅ | `upload(f) progress -> u { … }` — 스트림 + `pump()` ✅ |
| 16 | 오프라인 | ❌ | `net.is_online()` |
| 17 | 백그라운드 작업 큐 | ❌ | 클로저 내 `<-` (`unexpected token '<-'`) |
| 18 | 낙관적 리스트 조작 | ❌ | `rollback <-` + `<Check>` |
| 19 | 리치 텍스트 에디터 | ✅ | `<Raw>` 탈출구 |
| 20 | 스냅샷/시간여행 | ⚠️ | `state = timeline[cursor]` ✅, `on_change(state)`(after 없음)·`<DevToolbar>` ❌ |

---

## 5. 다음 작업 우선순위 제안

> v0.4에서 **1~4, 7**을 구현했다(2.5절). 남은 순위는 아래 5·6번 항목이다.

| 순위 | 작업 | 이유 |
|---|---|---|
| ~~1~4~~ | ~~매크로 버그 픽스 / 시간 리터럴 / 아이템 캡처 / 어휘 확장~~ ✅ v0.4 | — |
| ~~5~~ | ~~**`#[store]` + keyed 트리**~~ ✅ v0.5 | — |
| ~~6~~ | ~~**구독 계열**(`on_message`/`on_event`/`on_net_change`/`on_navigate`/`on_unmount`)~~ ✅ v0.5 | — |
| ~~7~~ | ~~**콜백 prop + 사용자 컴포넌트 children**~~ ✅ v0.5 | — |
| 8 (→최우선) | **리스트 아이템 편집** — 인덱스 추적(`t.done = !t.done` → `items[i].done` 전개) + 아이템당 핸들러 여러 개(`Rc` 바인딩) | 사양서 3.3의 마지막 구멍. `<Check checked={t.done} on_change={t.done = !t.done}>`가 목표 |
| 9 | **어휘 2차 확장** — `Table`, `VirtualList`, `Scroll`, `Plot`, `Dock`, `Window`, `Sidebar`, `Menu/Item`, `Fade`, 소문자/HTML 태그 | 프로토타입 `2.rs` 대부분 |
| 10 | **서비스 객체 + `#[view]` 속성 매크로** — `ws`/`api`/`cache`, `#[view] fn …` 문법 | 사양서 표면 문법과의 마지막 차이 |
| 11 | **플랫폼 확장** — `Desktop`/`Web`/`Terminal`, `mount(엘리먼트)`, `memo!`, i18n | 사양서 7장/14장 |
| 12 | **스냅샷 계약** — `Element: Serialize` ✅ **선택 기능**(`--features serde`, 2.8절) + insta | 사양서 8.3 |

---

## 6. 부록 — 재현 방법

### 6.1 컴파일 검증 스크립트

```sh
cd /home/ubuntu/projects/elm/elm && cargo build
LIB=$(ls target/debug/deps/libelm_magic-*.rlib | head -1)
mkdir -p /tmp/elmchk && cd /tmp/elmchk
# 검증 스니펫(s01_*.rs 등)을 배치한 뒤:
rustc --edition 2021 --test --emit=metadata \
  -L dependency=/home/ubuntu/projects/elm/elm/target/debug/deps \
  --extern elm_magic=$LIB s17_attr_after.rs -o /tmp/elmchk/out.rs
```

> `--test` 없이 `--crate-type lib`로 컴파일하면 `#[test]` 본문이 `cfg(test)`로 건너뛰어져
> `app.mock(...)` 같은 오류를 놓친다. **반드시 `--test`를 붙일 것.**

### 6.2 대표 실패 스니펫

> v0.4 반영 후 재측정: 스니펫 58개 중 **33개 통과** (이전 11개). 아래 표의 항목 중
> **1~6, 10, 12, 14번(3.1절)** 관련 행은 이제 통과한다. 남은 실패는 대부분
> "의도적 미구현"(3.2~3.6절) 또는 스니펫 자체의 오류다.

| 파일 | 스니펫(핵심) | 결과 |
|---|---|---|
| `s01_tick_ms.rs` | `on_tick(16ms) { … }` | `invalid suffix 'msu64' for number literal` |
| `s17_attr_after.rs` | `on_change(query) after 300ms { … }` | `invalid suffix 'msu64' for number literal` |
| `s02_h1.rs` | `<h1>"title"</h1>` | `proc macro panicked` (unknown tag) |
| `s03_children.rs` | `<Modal on_close={open = false}><Text>"…"</Text></Modal>` | `proc macro panicked` (cannot have children) |
| `s04_comma_stmt.rs` | `on_click={sort = Sort::Age, asc = !asc}` | `E0070` invalid left-hand side of assignment |
| `s05_prop_fn.rs` | `fn Row1(item = String::new(), on_select: fn(String))` | `proc macro panicked` (must have a default value) |
| `s06_store.rs` | `#[store] struct App { theme: String }` | `cannot find attribute 'store'` |
| `s07_check.rs` | `<Check checked={done} on_change={done = !done} />` | `E0422` `CheckProps` not found |
| `s09_on_message.rs` | `on_message(ws, m) { … }` | `E0425` `on_message` not found |
| `s10_mock_chain.rs` | `app.mock(search_api, \|q: String\| …)` | `E0599` no method named `mock` |
| `s11_assert_api.rs` | `app.type_("input", "rust"); app.assert_text(…);` | `E0061` / `E0599` |
| `s12_mount_element.rs` | `mount(ui! { <Counter /> })` | `E0425` `__elm_ctx` not found / 인자 개수 오류 |
| `s13_platforms.rs` | `elm_magic::run(Desktop, App)` | `E0425` `Desktop` not found |
| `s14_mount_after.rs` | `on_mount { user <- api_user(id) after 5min }` | `expected …, found 'after'` |
| `s18_view_attr.rs` | `#[view] fn Counter(n = 0) { … }` | rustc 파싱 오류 (`found '='`) |
| `s19_spinner.rs` | `{if loading { <Spinner /> } else { … }}` | `E0422` `SpinnerProps` not found |
| `s20_match_arms.rs` | `St::Failed(e) => <Banner>{e}</Banner>` | `proc macro panicked` |
| `s21_submit.rs` | `on_click={submit(name.clone(), email.clone())}` | `expected expression, found ';'` |
| `s24_underscore_click.rs` | `on_click={items.push(_, c)}` | `E0425` `_elm_v` not found |
| `s26_memo.rs` | `memo!(items.iter().sum::<f64>())` | `cannot find macro 'memo'` |
| `s27_on_navigate.rs` | `on_navigate(\|r\| route = r)` | `E0425` `on_navigate` not found |
| `s28_closure_effect.rs` | `let start = \|\| { … run <- spawn_worker(job) };` | `unexpected token '<-'` |
| `s31_effect_after.rs` | `on_click={fresh <- api_user(id) after 5min}` | `expected …, found 'after'` |
| `s33_rollback.rs` | `items.iter().map(\|t\| <Button on_click={rollback <- toggle(t.id)}>)` | `E0716` temporary dropped |
| `s34_table.rs` | `<Table virtualized …>` | `proc macro panicked` |
| `s35_textarea.rs` | `<TextArea value={text} on_change={text = _} />` | `E0422` `TextAreaProps` not found |
| `v1.rs` | `{(0..n).map(\|i\| <Row>"{i}"</Row>)}` | `E0425` `n` not found |
| `v5.rs` | `on_click={items.remove(0)}` | `E0308` (반환값 `String` vs `()`) |
| `t4.rs` | `{if loading { <Text>"l"</Text> } else { (0..n).map(…) }}` | `E0308` `if`/`else` 타입 불일치 |
| `t5.rs` | `on_enter={items.push(Todo { text })}` | `expected ',', found 'text'` |
| `w2.rs` | `items.map(\|t\| <Button on_click={sel = t.clone()}>"x"</Button>)` | `E0716` temporary dropped |
| `w3.rs` | `on_change(s) { tl.push(cursor) }` | `proc macro panicked` (after 필요) |
| `w4.rs` | `<Text strike={done}>"x"</Text>` | ✅ 통과 — **속성이 조용히 무시됨** |

### 6.3 통과한(대조군) 스니펫

| 파일 | 스니펫 | 결과 |
|---|---|---|
| `t1.rs` | `on_click={submit(name, email)}` | ✅ |
| `t3.rs` | `on_mount { if flag { user <- api_user(id) } }` | ✅ |
| `t6.rs` | `let total = items.iter()…; {items.map(…)} "Subtotal: {total}"` | ✅ |
| `s08_class_cond.rs` | `<Text class={if done { "done" } else { "" }}>"x"</Text>` | ✅ |
| `s16_ui_in_map.rs` | `{items.map(\|t\| ui! { <Row>"{t}"</Row> })}` | ✅ |
| `s23_ui_state.rs` | `{ui! { <Text>"Count: {n}"</Text> }}` | ✅ |
| `s32_stream_named.rs` | `{upload(String::new()) -> pct { }}` | ✅ |
| `s39_router_match.rs` | `{match route { … => ui! { <Home /> } }}` | ✅ |
| `w5.rs` | `<Button disabled={n > 0} on_click={n = 0}>` | ✅ |

### 6.4 기존 테스트 현황 (워크스페이스 90개 통과)

```
tests/counter.rs   tests/todo.rs    tests/effects.rs   tests/lifecycle.rs
tests/css.rs       tests/mock.rs    tests/stream.rs    tests/raw.rs
tests/platform.rs  tests/syntax.rs  tests/timers.rs    tests/widgets.rs
tests/sugar.rs     tests/store.rs   tests/subs.rs      tests/callbacks.rs
                   tests/integration.rs
                   crates/elm-magic-egui/tests/adapter.rs
```

- `tests/syntax.rs` — 매크로 문법 픽스 7건 (범위/메서드/컴마/축약/분기 통일/캡처/소유 반복)
- `tests/timers.rs` — 시간 리터럴, `after` 없는 `on_change`, 지연 효과 + mock
- `tests/widgets.rs` — 신규 태그 11종 + `<Check>` 토글 + `<TextArea>` 입력 + `<Tab>`/`<Th>` 클릭
- `tests/sugar.rs` — `.mock()` / `.mock2()`, `assert_text` / `assert_visible` / `assert_hidden`
- `tests/store.rs` — `#[store]` 공유·버전, keyed 재정렬 상태 보존, unmount 초기화 + `on_unmount`, `store_fn!` 기본값
- `tests/subs.rs` — `on_message`(+스트림 목), 이벤트 버스, `on_navigate`(String/타입), `on_net_change`
- `tests/callbacks.rs` — 콜백 prop(`fn(T)`)·`_` 전달값·필수 prop panic, 컴포넌트 children(중첩 포함)
- `tests/integration.rs` — store + keyed + 구독 + 콜백 + children 를 한 트리에서 통합 검증

---

## 7. 결론

- `spec.md` / `1.rs`~`3.rs`는 **목표 상태(설계 의도)** 이고, `README.md`는 구현된 범위와 v0.x 제한을 정직하게 기술하고 있다.
- v0.4에서 **문서화되지 않았던 매크로 버그 16종 중 8종을 수정**하고 어휘를 11종 늘렸다.
- v0.5에서 **우선순위 5·6·7**을 구현해 사양서의 **전역 상태(4.2)·keyed 트리(9.5)·구독(5.3/5.4)·콜백 prop/children(3.1)** 이 모두 동작한다.
  - `#[store]` + 이름 기반 슬롯 + 버전 카운터, 인스턴스 경로 keyed 슬롯과 unmount,
  - `on_message`(+스트림 목) / `on_event`+`bus.emit` / `on_net_change`+`net.is_online()` / `on_navigate` / `on_unmount`,
  - `on_select: fn(Id)` 콜백 prop과 컴포넌트 children, 클릭 가능한 컨테이너.
- 남은 병목은 **리스트 아이템 편집(인덱스 추적)** 과 **어휘 2차 확장**, **서비스 객체/`#[view]` 문법**이며,
  이는 `README.md`의 v0.6 로드맵과 일치한다.

