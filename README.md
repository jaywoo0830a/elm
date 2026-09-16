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

| 문법 | 의미 | v0.1 |
|---|---|---|
| **변수** = 상태 | `fn Counter(n = 0)`의 매개변수가 아레나 슬롯으로 승격 | ✅ |
| **대입** = 이벤트 | `on_click={n += 1}`이 슬롯 변경으로 컴파일 | ✅ |
| **태그** = 뷰 | `<Col>`, `<Row>`, `<Text>`, `<Button>`, `<Input>` | ✅ |
| **`<-`** = 효과 | async / 스트림 / 스폰 | v0.2 |
| **`mock()`** = 테스트 | 런타임 교체 · 시간 진행 | v0.2 |

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

## v0.1 제한

- 속성 매크로 `#[view]` 대신 함수형 매크로 `view! { fn ... }` 사용
  (매개변수 기본값 `n = 0`은 rustc 파서가 거부하는 비-Rust 문법이므로
  토큰을 원본 그대로 받는 함수형 매크로가 필요).
- `<-` 효과, `on_mount`/`on_key`, `css!`, `#[store]`는 v0.2+ 로드맵.
- 중첩 컴포넌트의 상태 슬롯은 렌더 순서 기반 오프셋으로 분리
  (조건부로 등장하는 컴포넌트 순서가 바뀌면 슬롯이 섞일 수 있음 — keyed 트리는 v0.2+).

