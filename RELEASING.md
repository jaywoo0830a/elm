# 릴리스 절차 (Releasing)

crates.io에 **`elm-magic` / `elm-magic-macros` / `elm-magic-egui`** 세 크레이트를
**순서대로** 배포한다. 버전은 항상 셋이 함께 올라간다. (배포판: 0.1.0 → 다음: 0.5.0)

> ⚠️ **버전 결정 필요** — 매니페스트는 아직 `0.5.0`인데 트리에는 **v0.6 스타일 작업**이 들어 있다
> (`CHANGELOG.md`의 `[Unreleased] — v0.6 스타일`, `README.md`의 `v0.6` 절).
> 둘 중 하나를 고른다:
>
> 1. **0.5.0을 먼저 배포** — 스타일 작업 이전 커밋에서: `git tag v0.5.0 d063fb2` (그 커밋의 매니페스트가 0.5.0이다).
> 2. **0.6.0으로 올려서 배포** — 1번 절의 5곳을 `0.6.0`으로 바꾸고 `CHANGELOG`의 `[Unreleased]`를 `[0.6.0]`으로 옮긴다.
>
> 어느 쪽이든 **매니페스트 버전 = 배포 버전**이 되게 맞춘 뒤 진행한다.

## 0. 준비

```sh
cargo login            # crates.io API 토큰 (또는 CARGO_REGISTRY_TOKEN 환경변수)
cargo search elm-magic # 현재 배포된 버전 확인
```

## 1. 버전 올리기 — 5곳을 모두 맞춘다

| 파일 | 항목 |
|---|---|
| `Cargo.toml` | `[package] version` |
| `Cargo.toml` | `elm-magic-macros = { path = …, version = "0.5.0" }` |
| `crates/elm-magic-macros/Cargo.toml` | `[package] version` |
| `crates/elm-magic-egui/Cargo.toml` | `[package] version` |
| `crates/elm-magic-egui/Cargo.toml` | `elm-magic = { path = "../..", version = "0.5.0" }` |

`Cargo.lock`은 `cargo build`가 자동 갱신한다.
`README.md` 설치 스니펫(`elm-magic = "0.5"`)과 `CHANGELOG.md`도 함께 갱신한다.

## 2. 로컬 검증 (배포 전 필수 — 되돌릴 수 없다)

```sh
# 테스트: 기본(의존성 0) + 전체 기능
cargo test --workspace                  # 127
cargo test --workspace --all-features   # 131

# 패키지 검증
cargo package --allow-dirty -p elm-magic-macros

# 아직 배포되지 않은 0.5.0 의존성은 로컬 patch로 대체해 **미리** 검증한다.
cargo package --allow-dirty -p elm-magic \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'

cargo package --allow-dirty -p elm-magic-egui \
  --config 'patch.crates-io.elm-magic.path="."' \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'
```

- `--config patch…`는 **검증 전용**이다. 실제 `Cargo.toml`에 patch를 남기지 않는다.
- 이걸 빼고 `-p elm-magic`을 돌리면 아직 0.5.0이 없어서
  `failed to select a version for the requirement elm-magic-macros = "^0.5.0"`이 난다 — **정상**이다.

## 3. 커밋 & 태그

```sh
git add -A
git commit -m "chore: release v0.5.0"
git tag v0.5.0
git push origin HEAD --tags
```

## 4. 배포 — 순서가 중요하다

의존 방향: `elm-magic-macros` → `elm-magic` → `elm-magic-egui`

```sh
cargo publish -p elm-magic-macros

# 인덱스 전파 대기(보통 1~3분). 확인:
cargo search elm-magic-macros

cargo publish -p elm-magic
cargo publish -p elm-magic-egui
```

- `elm-magic`은 `elm-magic-macros = "0.5.0"`을 요구한다. 매크로가 먼저 올라가야 한다.
- 위 2번의 patch 검증은 **로컬 전용**이라, 실제 배포는 이 순서를 지켜야 통과한다.
- `cargo publish --dry-run`은 레지스트리 조회가 필요하다. 오프라인이면
  `attempting to make an HTTP request, but --offline was specified`로 실패하니 온라인에서 실행한다.

## 5. 배포 후 확인

```sh
cargo search elm-magic
cargo add elm-magic@0.5.0 elm-magic-egui@0.5.0   # 새 임시 프로젝트에서
cargo test
```

- docs.rs: <https://docs.rs/elm-magic/0.5.0> — `all-features`로 빌드되어 `serde` 기능도 보인다
  (`[package.metadata.docs.rs] all-features = true`).
- 문제가 생겨도 **삭제는 불가**하다. 72시간 내 `cargo yank --version 0.5.0 -p <crate>`로
  양키(새 프로젝트의 의존 해석에서 제외)만 가능하니, 2번 검증을 반드시 통과시키고 올린다.

## 체크리스트

- [ ] 3개 `Cargo.toml` 버전 + 2개 path 의존성 제약을 모두 올렸다
- [ ] `Cargo.lock`이 0.5.0으로 갱신됐다 (`grep -A1 'name = "elm-magic"' Cargo.lock`)
- [ ] README 설치 스니펫 · CHANGELOG 갱신
- [ ] `cargo test --workspace` / `--all-features` 통과
- [ ] `cargo package` 3개 통과 (patch 사전 검증 포함)
- [ ] 커밋 + `git tag v0.5.0` + push
- [ ] `elm-magic-macros` → (전파 대기) → `elm-magic` → `elm-magic-egui`
- [ ] `cargo add` 스모크 테스트, docs.rs 확인

## 선택 사항

- `rust-version`(MSRV)을 명시하면 구 툴체인에서 친절한 에러가 난다.
  최고 요구 API가 `Waker::noop()`(Rust 1.85)이라 `rust-version = "1.85"`가 유효하다.
- CI(`.github/workflows/ci.yml`)에서 `cargo test --workspace --all-features` +
  `cargo package` 3종을 돌리면 이 문서의 2번을 자동화할 수 있다.
