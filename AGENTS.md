# Agent instructions

For AI agents (Claude Code, Codex, ...) and people working in this
repository. The data repository `openparts-data` (checked out next to
this one) has its own CONTRIBUTING.md.

## Before every commit or push: run what CI runs

CI (`.github/workflows/ci.yml`) rejects a push on the first failing
step, so run these from the repository root and fix every failure
before you commit. A green `cargo test` alone is not enough: most past
CI failures were formatting or stale generated files.

```sh
cargo fmt --all -- --check                    # fix with: cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p openparts-viewer --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace
cargo run -q -p openparts-cli -- validate --data ../openparts-data
web/scripts/gen-api-types.sh && git diff --exit-code -- web/api
```

If you touched `web/`, also run in `web/`:

```sh
moon check --target js --deny-warn
moon test --target js
bun run build
```

- Run `cargo fmt --all` over the whole workspace, not only the crate
  you edited: the check covers every crate.
- After changing a type that `openparts-server` serializes, regenerate
  `web/api/types.mbt` (`web/scripts/gen-api-types.sh`) and commit it.
- After changing the dataset rules, the whole `openparts-data` checkout
  must still validate; `openparts data fmt --check .` must pass there too.
  Run `openparts data fmt` only with explicit file arguments.
- Tests that need SolidCraft skip when it isn't installed; set
  `OPENPARTS_REQUIRE_SOLIDCRAFT=1` where it is.

## Shared machines

When several agents build at once, share one target directory and
limit jobs (`CARGO_TARGET_DIR=target/dev2 CARGO_BUILD_JOBS=4`) so the
disk and CPU don't run out.
