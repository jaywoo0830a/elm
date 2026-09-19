# Releasing

Publishes four crates to crates.io — **`elm-magic` / `elm-magic-macros` /
`elm-magic-egui` / `elm-magic-gpui`** — and always keeps their versions in lockstep.
(Published: 0.1.0, 0.5.0, 0.6.0, 0.6.1, 0.7.0, 0.7.1, 0.7.2, 0.7.3 → next: **0.7.4**)

## 0. Prepare

```sh
cargo login            # crates.io API token (or CARGO_REGISTRY_TOKEN)
cargo search elm-magic # check the currently published version
```

## 1. Bump the version — keep these 7 places in sync

| File | Field |
|---|---|
| `Cargo.toml` | `[package] version` |
| `Cargo.toml` | `elm-magic-macros = { path = …, version = "0.7.4" }` |
| `crates/elm-magic-macros/Cargo.toml` | `[package] version` |
| `crates/elm-magic-egui/Cargo.toml` | `[package] version` |
| `crates/elm-magic-egui/Cargo.toml` | `elm-magic = { path = "../..", version = "0.7.4" }` |
| `crates/elm-magic-gpui/Cargo.toml` | `[package] version` |
| `crates/elm-magic-gpui/Cargo.toml` | `elm-magic = { path = "../..", version = "0.7.4" }` |

`Cargo.lock` updates itself on the next `cargo build`.
Also refresh `README.md`'s install snippet (`elm-magic = "0.7"`) and `CHANGELOG.md`.

`elm-magic-gpui` was **first published in 0.7.1** — for that release only, the
"published versions" list in the header above grew from three crates to four.

## 2. Local verification (mandatory before publishing — it cannot be undone)

```sh
cargo test --workspace                  # 285
cargo test --workspace --all-features   # 292
cargo test --workspace --release        # 285 (오버플로 의미론 포함)

cargo package --allow-dirty -p elm-magic-macros

# The not-yet-published 0.7.4 dependencies are substituted with local patches so that
# packaging can be verified *ahead of time*.
cargo package --allow-dirty -p elm-magic \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'

cargo package --allow-dirty -p elm-magic-egui \
  --config 'patch.crates-io.elm-magic.path="."' \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'

cargo package --allow-dirty -p elm-magic-gpui \
  --config 'patch.crates-io.elm-magic.path="."' \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'
```

- The `--config patch…` flags are **verification only** — never leave patches in `Cargo.toml`.
- Without them `-p elm-magic` fails with
  `failed to select a version for the requirement elm-magic-macros = "^0.7.4"` because 0.7.4
  is not on crates.io yet. That is expected.
- `--allow-dirty` is fine here: the working tree holds the version bump plus generated files
  (and `README.md` is a work in progress).

## 3. Commit & tag

```sh
git add -A
git commit -m "chore: release v0.7.4"
git tag v0.7.4
git push origin HEAD --tags
```

## 4. Publish — order matters

Dependency direction: `elm-magic-macros` → `elm-magic` → `elm-magic-egui` / `elm-magic-gpui`

```sh
cargo publish -p elm-magic-macros

# wait for index propagation (usually 1-3 minutes). check:
cargo search elm-magic-macros

cargo publish -p elm-magic
cargo publish -p elm-magic-egui
cargo publish -p elm-magic-gpui   # first published in 0.7.1, no propagation wait needed
```

- `elm-magic` requires `elm-magic-macros = "0.7.4"`, so the macro crate must go first.
- Both adapters require `elm-magic = "0.7.4"`, so they go last — after the index has
  propagated (check `cargo search elm-magic`).
- The patch verification in step 2 is local-only; the real publish must use this order.
- `cargo publish --dry-run` needs registry access. Offline it fails with
  `attempting to make an HTTP request, but --offline was specified` — run it online.
- `cargo publish` runs the same packaging checks as `cargo package`, so step 2 is the gate.

## 5. After publishing

```sh
cargo search elm-magic
cargo add elm-magic@0.7.4 elm-magic-egui@0.7.4   # in a scratch project
cargo test
```

- docs.rs: <https://docs.rs/elm-magic/0.7.4> — built with `all-features`, so the `serde`
  feature is documented (`[package.metadata.docs.rs] all-features = true`).
- Mistakes cannot be deleted. Within 72 hours you can only
  `cargo yank --version 0.7.4 -p <crate>` (excludes it from new dependency resolution),
  so make step 2 pass before publishing.

## Checklist

- [ ] 4 × `Cargo.toml` versions + 3 × path dependency requirements bumped
- [ ] `Cargo.lock` shows 0.7.4 (`grep -A1 'name = "elm-magic"' Cargo.lock`)
- [ ] README install snippet (`0.7`) · CHANGELOG updated
- [ ] `cargo test --workspace` / `--all-features` / `--release` pass (285 / 292 / 285)
- [ ] `tests/compile_fail/ui/*.stderr`와 `tests/snapshots/*.snap`가 커밋됐는지
      (툴체인을 올렸다면 `TRYBUILD=overwrite` / `INSTA_UPDATE=always`로 재생성 후 diff 리뷰)
- [ ] `cargo package` passes for all 4 (including the patch verification)
- [ ] commit + `git tag v0.7.4` + push
- [ ] `elm-magic-macros` → (wait for propagation) → `elm-magic` →
      `elm-magic-egui` / `elm-magic-gpui`
- [ ] `cargo add` smoke test, docs.rs check

## Optional

- Declaring `rust-version` (MSRV) produces friendlier errors on old toolchains.
  The newest API we need is `Waker::noop()` (Rust 1.85), so `rust-version = "1.85"` is valid.
  (`.github/workflows/ci.yml` already checks 1.85 with `cargo check`.)
- CI (`.github/workflows/ci.yml`) automates step 2 — three test modes, a fmt/clippy gate
  for the quality tests, snapshot approval, the zero-runtime-dependency check, the four
  `cargo package` invocations, a Windows/macOS core+egui job, and the MSRV check.
  Known pre-existing drift it deliberately does **not** gate on: `src/style.rs` is not
  `cargo fmt` clean, and `src/` carries clippy lints (see `CHANGELOG.md` [Unreleased]).

## Version history note

- The tree carried the v0.6 style work while the manifests still said `0.5.0`
  (see `CHANGELOG.md`). That decision is now settled: **the style milestone ships as 0.6.0**,
  so the manifests are at `0.6.0` and `[Unreleased]` was folded into `[0.6.0]`.
  `git tag v0.5.0` still points at the earlier commit (`d063fb2`) whose manifests were `0.5.0`.
- **0.6.1 is a patch**: the FreeDF bug report's fourth item — `css!` silently dropped
  hyphenated class selectors (BEM modifiers like `.tabs__item--active`). No API change,
  no new features; only `join_selector` / `validate_selector` (macros) and
  `style::register` (core) plus `tests/bug_report.rs` regression tests.
- **0.7.1 is a patch**: the second FreeDF bug report (`elm-magic-bug-report.md`, six
  items) is **not** in this release — its header says `0.7.2에서 수정 예정`. 0.7.1 ships
  the `on_change` fixes (`crates/elm-magic-macros/src/jsx.rs`) and the new
  `elm-magic-gpui` adapter only. The report's six items all live in `view.rs` /
  `jsx.rs`, so 0.7.2 will be macros-only.
- **0.7.2 is a patch**: the second report's six items are fixed — macros only
  (`crates/elm-magic-macros/src/view.rs` + `jsx.rs`; no core/adapter change). The
  report file was deleted and its repros live on in `tests/bug_report.rs` (items 5–10).
- **0.7.3 is a patch**: a self-audit of local/global **state lifetimes** (`state.rs`,
  `lib.rs::frame`, `store.rs`, `jsx.rs`): writes to unmounted slots are dropped,
  subscriptions stop at unmount, store keys are module-qualified, and a store write
  during render triggers one more render. New tests in `tests/state.rs`. API is
  additive only (`State::is_alive`, `Arena::spawn_stream_while` /
  `spawn_mockable_stream_while`, `Arena::store_writes`).
- **0.7.4 is a patch**: the third bug report's remaining items 11–13. Core + macros
  + egui adapter: value props are live until the child writes them
  (`Arena::slot_dirty`, `Ctx::slot_prop`), egui `Button`/`Tab` honour CSS
  `padding`/`height`, and `css!` self-registration uses the platform's init section
  (`.CRT$XCU` / `__DATA,__mod_init_func` / `.init_array`) with `style::init_styles()`
  as the explicit fallback. API additive only (`style::init_styles`; the report file
  was deleted and its repros live on in `tests/bug_report.rs` + `tests/init_styles.rs`).
- The 0.7.0 CHANGELOG entry was **restored to exactly what 0.7.0 published** (152 / 156
  tests) when 0.7.1 was cut; the delta moved up into `[0.7.1]`. Published 0.7.0 was
  tagged from the `0.7` / `dev` branch, not from this one.