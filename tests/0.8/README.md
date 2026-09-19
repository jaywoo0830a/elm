# 0.8.0 테스트 — 조건/반복 (구현 완료)

`0.8.0`의 구현 범위는 **조건/반복**뿐이다 (`0.8-preview.md` §0의 단계 1 "문법 팩" 중
제어 흐름 태그). 이 디렉터리의 테스트가 그 계약이며, 현재 **전부 green**이다
(`cargo test -p elm-magic`).

> 디렉터리 이름이 `0.8`이라 Rust 크레이트 이름으로 쓸 수 없어, 각 파일은 루트
> `Cargo.toml`의 `[[test]]` 항목으로 명시 등록한다 (`name = "v0_8_*"`).

## 구현된 문법

| 태그 | 파일 | 계약 |
|---|---|---|
| `<If when={c}>` / `<Else>` | `if_else.rs` | 참/거짓 분기, Else 생략 시 미렌더, 중첩, 동적 상태, 본문 전체 |
| `<For each={..} as={..} key={..}>` | `for_loop.rs` | 위치 기반/keyed 반복, 이터레이터, 빈 목록, 삭제 |
| `<Switch on={..}>` / `<Case when={..}>` / `<Default>` | `switch.rs` | 패턴 분기, Default 생략 시 미렌더, 상태 변화 반영 |
| `<> … </>` | `fragment.rs` | 레이아웃 래퍼 없이 자식 묶기 |
| (호환) | `interop.rs` | 기존 `{if}` / `.map()` 문법과 동일 결과 |

## 판정 기준 (acceptance)

- 새 태그는 **레이아웃 노드를 만들지 않는다** — `<If>`/`<For>`/`<Switch>`/`<>`가 감싼
  자식만 최종 트리에 나타난다.
- `<For>`는 `key`가 없으면 **위치 기반**, 있으면 **keyed 슬롯**을 발급한다
  (keyed 자식 상태가 키를 따라가고, 위치 기반은 따라가지 않음을 검증).
- `as={t}`로 바인딩한 아이템은 본문(자식 태그/이벤트 핸들러/문자열 보간)에서 쓸 수 있다.
- `<Switch>`의 `when={..}`은 **패턴**이다 (`Status::Failed(e)` 바인딩 허용).
- 기존 0.7 문법은 그대로 동작한다 (추가 중심).

## 실행

```bash
cargo test --test v0_8_if_else
cargo test --test v0_8_for_loop
cargo test --test v0_8_switch
cargo test --test v0_8_fragment
cargo test --test v0_8_interop

# 전체 (기본 스위트 포함)
cargo test -p elm-magic
```

0.8.0에서 다루지 않는 `IntoView`(§3.2), `bind`(§3.3), `cls!`(§3.5), `style!`(§3.6),
`<Await>`(§4.2), `widgets!`/`Bridge`(§6)는 후속 버전 테스트로 남긴다.

## 구현 위치

`crates/elm-magic-macros/src/jsx.rs` — `is_control_tag`/`emit_control`(`emit_if`,
`emit_for`, `emit_switch`)/`parse_fragment`와 `scan_tag_children`·`scan_named_block`
스캐너. 새 태그는 `Vec<Element>`를 내보내 `IntoElements`가 그대로 평탄화한다.