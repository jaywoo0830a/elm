# Releasing

Publishes three crates to crates.io — **`elm-magic` / `elm-magic-macros` / `elm-magic-egui`** —
and always keeps their versions in lockstep. (Published: 0.1.0, 0.5.0, 0.6.0 → next: **0.6.1**)

## 0. Prepare

```sh
cargo login            # crates.io API token (or CARGO_REGISTRY_TOKEN)
cargo search elm-magic # check the currently published version
```

## 1. Bump the version — keep these 5 places in sync

| File | Field |
|---|---|
| `Cargo.toml` | `[package] version` |
| `Cargo.toml` | `elm-magic-macros = { path = …, version = "0.6.1" }` |
| `crates/elm-magic-macros/Cargo.toml` | `[package] version` |
| `crates/elm-magic-egui/Cargo.toml` | `[package] version` |
| `crates/elm-magic-egui/Cargo.toml` | `elm-magic = { path = "../..", version = "0.6.1" }` |

`Cargo.lock` updates itself on the next `cargo build`.
Also refresh `README.md`'s install snippet (`elm-magic = "0.6"`) and `CHANGELOG.md`.

## 2. Local verification (mandatory before publishing — it cannot be undone)

```sh
cargo test --workspace                  # 138
cargo test --workspace --all-features   # 142

cargo package --allow-dirty -p elm-magic-macros

# The not-yet-published 0.6.1 dependencies are substituted with local patches so that
# packaging can be verified *ahead of time*.
cargo package --allow-dirty -p elm-magic \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'

cargo package --allow-dirty -p elm-magic-egui \
  --config 'patch.crates-io.elm-magic.path="."' \
  --config 'patch.crates-io.elm-magic-macros.path="crates/elm-magic-macros"'
```

- The `--config patch…` flags are **verification only** — never leave patches in `Cargo.toml`.
- Without them `-p elm-magic` fails with
  `failed to select a version for the requirement elm-magic-macros = "^0.6.1"` because 0.6.1
  is not on crates.io yet. That is expected.
- `--allow-dirty` is fine here: the working tree holds the version bump plus generated files
  (and `README.md` is a work in progress).

## 3. Commit & tag

```sh
git add -A
git commit -m "chore: release v0.6.1"
git tag v0.6.1
git push origin HEAD --tags
```

## 4. Publish — order matters

Dependency direction: `elm-magic-macros` → `elm-magic` → `elm-magic-egui`

```sh
cargo publish -p elm-magic-macros

# wait for index propagation (usually 1-3 minutes). check:
cargo search elm-magic-macros

cargo publish -p elm-magic
cargo publish -p elm-magic-egui
```

- `elm-magic` requires `elm-magic-macros = "0.6.1"`, so the macro crate must go first.
- The patch verification in step 2 is local-only; the real publish must use this order.
- `cargo publish --dry-run` needs registry access. Offline it fails with
  `attempting to make an HTTP request, but --offline was specified` — run it online.
- `cargo publish` runs the same packaging checks as `cargo package`, so step 2 is the gate.

## 5. After publishing

```sh
cargo search elm-magic
cargo add elm-magic@0.6.1 elm-magic-egui@0.6.1   # in a scratch project
cargo test
```

- docs.rs: <https://docs.rs/elm-magic/0.6.1> — built with `all-features`, so the `serde`
  feature is documented (`[package.metadata.docs.rs] all-features = true`).
- Mistakes cannot be deleted. Within 72 hours you can only
  `cargo yank --version 0.6.1 -p <crate>` (excludes it from new dependency resolution),
  so make step 2 pass before publishing.

## Checklist

- [ ] 3 × `Cargo.toml` versions + 2 × path dependency requirements bumped
- [ ] `Cargo.lock` shows 0.6.1 (`grep -A1 'name = "elm-magic"' Cargo.lock`)
- [ ] README install snippet · CHANGELOG updated
- [ ] `cargo test --workspace` / `--all-features` pass (138 / 142)
- [ ] `cargo package` passes for all 3 (including the patch verification)
- [ ] commit + `git tag v0.6.1` + push
- [ ] `elm-magic-macros` → (wait for propagation) → `elm-magic` → `elm-magic-egui`
- [ ] `cargo add` smoke test, docs.rs check

## Optional

- Declaring `rust-version` (MSRV) produces friendlier errors on old toolchains.
  The newest API we need is `Waker::noop()` (Rust 1.85), so `rust-version = "1.85"` is valid.
- CI (`.github/workflows/ci.yml`) running `cargo test --workspace --all-features` plus the
  three `cargo package` invocations automates step 2.

## Version history note

- The tree carried the v0.6 style work while the manifests still said `0.5.0`
  (see `CHANGELOG.md`). That decision is now settled: **the style milestone ships as 0.6.0**,
  so the manifests are at `0.6.0` and `[Unreleased]` was folded into `[0.6.0]`.
  `git tag v0.5.0` still points at the earlier commit (`d063fb2`) whose manifests were `0.5.0`.
- **0.6.1 is a patch**: the FreeDF bug report's fourth item — `css!` silently dropped
  hyphenated class selectors (BEM modifiers like `.tabs__item--active`). No API change,
  no new features; only `join_selector` / `validate_selector` (macros) and
  `style::register` (core) plus `tests/bug_report.rs` regression tests.