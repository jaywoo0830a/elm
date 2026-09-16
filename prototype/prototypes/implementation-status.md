# elm-magic — 구현 현황 정리

> 기준: `spec.md`(사양서) + `1.rs` / `2.rs` / `3.rs`(각 20 패턴)
> 대상 구현: `src/`, `crates/elm-magic-macros/`, `crates/elm-magic-egui/`
> 작성 시점 기준 `git status` clean, 워크스페이스 테스트 **36개 통과**.

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
| 플랫폼 | `run(Headless, C) -> TestApp`, egui 어댑터 crate(`render(ui, &tree, &mut arena) -> Pass`, 6태그, `<Raw>` 실제 호출) | `src/platform.rs`, `crates/elm-magic-egui/{src,tests}/` |

---

## 3. 미구현 목록 ❌

### 3.1 매크로/문법 — 컴파일이 아예 안 되는 것 (전부 검증됨)

| # | 미구현 항목 | 대표 스니펫 | 실제 에러 | 원인 위치 |
|---|---|---|---|---|
| 1 | **시간 리터럴 `ms` / `min`** | `on_tick(16ms)`, `on_change(q) after 300ms`, `<- f() after 5min` | `invalid suffix 'msu64' for number literal` | `jsx.rs` — `{}u64`로 접미사 결합. **정수만 가능** |
| 2 | `..` 범위 뒤 상태 읽기 | `{(0..n).map(…)` | `cannot find value 'n'` | `jsx.rs:391` `prev_dot`이 `..`에서 켜짐 |
| 3 | 이벤트 본문에서 상태 메서드 **0-인자** 호출 | `on_click={submit(name.clone(), email.clone())}`, `if user.is_empty()` | `expected expression, found ';'` | `jsx.rs:682-713` — `n_args==0`이면 `let __elm_args = ;` |
| 4 | 이벤트 메서드 호출의 반환형 제약 / 요소 삭제 | `on_click={items.remove(t)}`, `items.remove(0)` | `E0308` (Vec::remove는 값 반환·인덱스 기반) | 사양서의 "값으로 삭제" API 없음 |
| 5 | 리스트 아이템을 이벤트에서 캡처 | `items.map(\|t\| <Button on_click={sel = t.clone()}>` | `E0716` temporary dropped while borrowed | `.clone().iter()`가 낸 `&T`를 `'static Rc`로 캡처 |
| 6 | 구조체 리터럴 축약 | `items.push(Todo { text })` | `expected ',', found 'text'` | `jsx.rs:723-738` — 중괄호를 통째로 치환해 `Todo text: (…)` 생성 |
| 7 | 컴포넌트 children | `<Modal><Text/></Modal>`, `<Tab …>"Home"</Tab>`, `<Window>…</Window>` | `proc macro panicked` ("cannot have children") | `jsx.rs:1190` |
| 8 | 콜백 prop(함수 타입) | `fn ItemRow(item: Item, on_select: fn(Id))` | panic `must have a default value` | `view.rs:166` |
| 9 | 소문자/HTML 태그 | `<h1>`, `<h2>` | panic `unknown tag` | `jsx.rs:1184` |
| 10 | 컴마 다중문 | `on_click={sort = Name, dir = dir.toggle()}` | `E0070` / `E0308` | 문장 구분은 `;`만 |
| 11 | `{_}`를 클릭류 이벤트에 사용 | `on_click={selected = Some(_)}`, `on_drop={cols.move_to(_, c)}` | `cannot find value '_elm_v'` | `jsx.rs:914-925` — 값 이벤트(on_change/on_enter)에만 바인딩 |
| 12 | `after` 없는 `on_change` | `on_change(state) { … }` (시간여행) | panic | `jsx.rs:187-255` |
| 13 | 클로저 내부 효과 | `let start = \|\| { … <- f() };` | `unexpected token '<-'` | 효과는 문장 위치만 |
| 14 | if/else 분기 타입 혼합 | `{if loading {<Spinner/>} else {users.map(…)}}` | `E0308` (Element vs iterator) | 사양서 3.4의 `IntoElements` 통일 미구현 |
| 15 | `#[view]` 속성 매크로 | `#[view] fn Counter(n = 0)` | rustc 파싱 단계에서 실패 (`=` 발견) | 비-Rust 문법 → 함수형 `view! { }`만 |
| 16 | 키워드 속성명 | `<Fade in={visible} duration={200}>` | panic | `in`이 키워드 |

### 3.2 태그 어휘 — 내장은 6개뿐

- **구현:** `Col`, `Row`, `Text`, `Button`, `Input`, `Raw`
- **미구현(프로토타입에 등장):**

  | 파일 | 미구현 태그 |
  |---|---|
  | `1.rs` | `Divider`, `Strong`, `Spinner`, `Banner`, `Check`, `Modal`, `Tab`, `Th`, `Td`, `Table`, `TextArea`, `DropZone`, `Card`, `Fade`, `ErrorBoundary`, `Theme`, `Layout`, `Sidebar`, `Main` |
  | `2.rs` | `Sidebar`, `Center`, `TopBar`, `BottomBar`, `NavMenu`, `MenuBar`, `Menu`, `Item`, `Sep`, `Dock`, `Scroll`, `Collapsing`, `Switch`, `Slider`, `Plot`, `Line`, `Points`, `HLine`, `Candlestick`, `Volume`, `Table(virtualized)`, `Column`, `Image`, `DragValue`, `Gradient`, `FileTree`, `Window`, `Progress` |
  | `3.rs` | `VirtualList`, `PostCard`, `Skeleton`, `UserCard`, `SplitPane`, `Kbd`, `DevToolbar`, `Status`, `ChatBubble`, `Check` |

- **속성이 조용히 버려지는 것**(오류 없음 → 더 위험): `<Row key={…}>`, `<Text strike={…}>`, `auto_focus`, `role`, `draggable`, `active`, `highlight`, `accept`
  → keyed diffing / a11y / 조건부 스타일 prop 미구현.

### 3.3 상태 모델

- `#[store]` / `app.theme` 전역 상태: **속성 매크로 자체가 없음** → `cannot find attribute 'store'`
- keyed 트리, 키 기반 재사용·unmount·초기화: 없음 (슬롯은 `ctx.base` 렌더 순서 오프셋 — `README.md` v0.1 한계에 명시)
- 사양서 4.4의 `State<T>: Deref / DerefMut` + 아레나 `unsafe` → 실제는 `Any` 다운캐스트 + `get/set/mutate`
- 사양서 9.1의 `Element<Msg>` + `Msg enum` 정적 디스패치 → 실제는 `Rc<dyn Fn>` 핸들러 (**트리 핫패스에 `dyn` 존재**)
- `Element: Serialize`, `StyleId(u32)` 인터닝 → 없음 (`Vec<String>` + `HashMap<String, …>`)
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
| ❌ 미구현 | `.mock(fn, impl)` 메서드 체인, `.assert_text` / `.assert_visible` / `.assert_hidden`, `type_("input", "rust")`(셀렉터 기반), `advance(300ms)`, `Cart(items: vec![…])` 생성자 슈가, `mount(ui! { <Counter /> })`(**엘리먼트 마운트** — `mount`는 `Component` 타입만), `insta` 스냅샷(사양서 8.3 형태), 시간여행 |

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

| 순위 | 작업 | 이유 |
|---|---|---|
| 1 | **매크로 버그 픽스(3.1절 1~4, 6, 10, 11, 12, 14)** — `..` 뒤 상태 치환, 이벤트 0-인자 메서드, 구조체 축약 중괄호, 컴마 문장 구분, 클릭류 `{_}`, `after` 선택화, 이벤트 메서드 반환값 무시, `if/else` 분기 `IntoElements` 통일 | 전부 매크로 코드 몇 줄 수준. `1.rs`의 다수 패턴과 사양서 3.3/3.4가 한 번에 살아남 |
| 2 | **시간 리터럴(`16ms`, `300ms`, `5min`)** 파서 + `<- … after <dur>` | 프로토타입 전반에 깔린 표기. 없으면 문서 예제가 그대로 안 돌아감 |
| 3 | **이벤트에서 리스트 아이템 캡처(3.1절 5)** — `Vec<Element>` 평탄화 시 `'static` 요구 제거(키/인덱스 기반 디스패치 또는 `Rc` 공유 값) | "리스트 + 행별 핸들러"는 UI의 최소 단위 기능 |
| 4 | **태그 어휘 1차 확장 + 컴포넌트 children** — `Spinner, Banner, Check, Modal, Divider, Strong, TextArea, Tab, Th/Td` + `props.children` | 프로토타입 태그 대부분이 여기서 막힘 |
| 5 | **`#[store]`(전역 상태) + keyed 트리** — 렌더 순서 오프셋 → 키 기반 diff | `README.md` v0.4 로드맵과 일치. 3.1절 5번과도 연결 |
| 6 | **구독 계열** — `on_message` / `on_event` / `on_unmount` / `on_navigate` + `mock`의 메서드·스트림 확장 | `3.rs` 절반의 전제 |
| 7 | **테스트 슈가** — `.mock()`, `assert_visible/hidden`, 셀렉터 `type_`, `mount(element)`, `Element: Serialize` + insta 스냅샷 | 사양서 8.x 예제가 그대로 돌게 됨 |

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

### 6.4 기존 테스트 현황 (워크스페이스 36개 통과)

```
tests/counter.rs   tests/todo.rs    tests/effects.rs   tests/lifecycle.rs
tests/css.rs       tests/mock.rs    tests/stream.rs    tests/raw.rs
tests/platform.rs  crates/elm-magic-egui/tests/adapter.rs
```

---

## 7. 결론

- `spec.md` / `1.rs`~`3.rs`는 **목표 상태(설계 의도)** 이고, `README.md`는 구현된 범위와 v0.x 제한을 상당 부분 정직하게 기술하고 있다.
- 문서와 구현의 차이 중 대부분은 **알려진 미구현**(전역 상태 · 구독 · 어휘 확장 · 플랫폼)이며, 이 문서에서 새로 정리한 것은 **문서화되지 않은 매크로 버그 16종(3.1절)** 과 **패턴별 판정(4절)** 이다.
- 다음 마일스톤의 실질적 병목은 "기능 추가"보다 **매크로 문법 버그 픽스 + 리스트/컴포넌트 children 지원**이며, 이 둘만 처리해도 프로토타입 `1.rs`의 대부분이 컴파일된다.

