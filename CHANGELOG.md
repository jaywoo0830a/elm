# CHANGELOG

## [Unreleased] — 0.8.0 조건/반복

### Added

- `<If when={..}>` / `<Else>` — 조건 분기. `<Else>` 생략 시 거짓이면 아무것도 그리지 않는다.
  중첩/동적 상태 변화/본문 전체 분기를 지원한다.
- `<For each={..} as={..} key={..}>` — 반복. `each`는 `IntoIterator`, `as`는 아이템 바인딩.
  `key`가 없으면 위치 기반, 있으면 keyed 슬롯(자식 상태가 키를 따라간다).
- `<Switch on={..}>` / `<Case when={..}>` / `<Default>` — 패턴 분기.
  `when`은 패턴이라 `Status::Failed(e)` 바인딩을 쓸 수 있다. `<Default>` 생략 시
  일치하는 분기가 없으면 아무것도 그리지 않는다.
- `<> … </>` — 레이아웃 없는 프래그먼트 (본문/조건/반복 안쪽 어디서나).
- 제어 흐름 태그는 **레이아웃 노드를 만들지 않는다** — 감싼 자식이 그대로 트리에 나온다.
  기존 `{if …}` / `{items.map(…)}` 문법과 공존한다 (추가 중심).

### Tests

- `tests/0.8/` 계약 테스트 5종(36개): `v0_8_if_else`, `v0_8_for_loop`, `v0_8_switch`,
  `v0_8_fragment`, `v0_8_interop`. 구현 현황은 `tests/0.8/README.md` 참고.

## [0.7.6] — 2026-09-19

### Changed

- fix bug
- ss

## [0.7.5] — 2026-09-19

### Changed

- add ss
- commit
- add some tests

