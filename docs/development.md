# 🧪 Development

> How to work on Panther Minor Controller itself: toolchain, running locally without hardware, quality checks, tests,
> commit rules, CI, releases, and the agent assets that automate recurring maintenance.

**Related:** [Architecture](architecture.md) · [API reference](api.md) · [Documentation index](README.md)

---

## 🧰 Toolchain

| Tool       | Version / config                         | Role                                        |
| ---------- | ---------------------------------------- | ------------------------------------------- |
| Rust       | stable, edition 2024 (`Cargo.toml`)      | Application                                 |
| Node.js    | `24.x` (`engines`)                       | Tooling only                                |
| pnpm       | `packageManager` in `package.json`       | Package manager and script runner           |
| Prettier   | `prettier.config.js` (`printWidth: 120`) | Markdown, JSON, TOML, YAML formatting       |
| rustfmt    | defaults                                 | Rust formatting                             |
| Clippy     | `-D warnings`                            | Rust linting                                |
| Lefthook   | `lefthook.yml`                           | Git hooks                                   |
| commitlint | `commitlint.config.js`                   | Conventional Commits enforcement            |
| Renovate   | `renovate.json`                          | Dependency updates, auto-merged after 1 day |

```bash
pnpm install   # installs the Git hooks outside CI
```

## 💻 Running locally

```bash
cargo run
```

On macOS (any non-Linux target) the relay runs in **simulation mode**: no GPIO access, each press prints a `[SIM]`
line and sleeps for its real duration. Set `STATUS_HOST` / `STATUS_PORT` to any reachable TCP service to exercise
the status probe, then open `http://localhost:8080`.

`pnpm run cargo:run` restarts on file changes; it needs [`cargo-watch`](https://crates.io/crates/cargo-watch).

## ✅ Quality checks

| Task                   | Command                                   | `pnpm` alias              |
| ---------------------- | ----------------------------------------- | ------------------------- |
| Build                  | `cargo build --workspace`                 | `pnpm run cargo:build`    |
| Type-check             | `cargo check --workspace`                 | `pnpm run cargo:check`    |
| Lint                   | `cargo clippy --workspace -- -D warnings` | `pnpm run cargo:clippy`   |
| Format Rust            | `cargo fmt`                               | `pnpm run cargo:fmt`      |
| Format everything else | `prettier --check --write .`              | `pnpm run prettier:write` |
| Test                   | `cargo test --workspace`                  | `pnpm run cargo:test`     |

### Git hooks

| Hook         | Runs                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------ |
| `commit-msg` | `commitlint`                                                                                                       |
| `pre-commit` | On `*.rs`: `cargo fmt`, `cargo check`, `cargo clippy -D warnings`; on MD/TOML/YAML/JSON: Prettier; fixes re-staged |

## 🧪 Testing

Tests live in `src/main.rs` (`mod tests`) and drive `handle_request` directly with a `MockRelay`.

| Rule                        | Detail                                                                            |
| --------------------------- | --------------------------------------------------------------------------------- |
| No real sleeps              | `MockRelay` records calls instantly; tests never wait for press durations         |
| Assert on calls, not timing | Use `call_count("short_press")` etc. to verify relay interaction                  |
| Guards are covered          | Every action has a success test and a `400` rejection test with zero relay calls  |
| Probe mode                  | `with_status_probe(host, port)` builds state where actions do not flip `power_on` |

```bash
cargo test --workspace   # all tests
cargo test power_on      # filter by name
```

## 📝 Rules

1. **Lint** — `cargo clippy --workspace -- -D warnings` must pass.
2. **Format** — never hand-format; run `cargo fmt` and `pnpm run prettier:write`.
3. **Errors** — return `AppError` through the `Result` alias; no panics in production paths.
4. **State guards** — every action endpoint returns `400` when the device is already in the target state.
5. **Commits** — [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/), e.g.
   `docs: restructure documentation into docs dir`.
6. **Docs** — follow the [documentation conventions](README.md#-documentation-conventions).

## 🔁 CI and releases

| Workflow                        | Trigger                           | Jobs                                                                                                                                   |
| ------------------------------- | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `.github/workflows/ci.yml`      | Push / PR to `main`               | `qa`: commitlint, `cargo fmt --check`, `prettier --check`, `cargo check`, Clippy; `test`: `cargo test`                                 |
| `.github/workflows/release.yml` | Push to `main` or manual dispatch | On a `chore(release): vX.Y.Z` commit: cross-compiles `aarch64-unknown-linux-gnu`, tags it, publishes a release with a commit changelog |

Each release publishes four assets: the `panther-minor-controller` binary and the three scripts from `scripts/`. The
release profile is size-optimized: `opt-level = "z"`, LTO, `strip`, one codegen unit.

Releases go through a `release/vX.Y.Z` branch: bump the version in `Cargo.toml`, `package.json` and the root README
quick start, commit as `chore(release): vX.Y.Z` and open a PR. Rebase-merging it is the release: `release.yml` creates
the tag on the merged commit, since a tag pushed with `GITHUB_TOKEN` would not trigger a workflow. Re-run a release with
`gh workflow run release.yml -f version=vX.Y.Z`. The `release` agent skill performs the whole sequence up to your merge.

## 🤖 Agent assets

| Asset                             | Purpose                                           |
| --------------------------------- | ------------------------------------------------- |
| `AGENTS.md`                       | Project context and rules for AI assistants       |
| `.github/copilot-instructions.md` | Copilot PR review guidance                        |
| `.agents/skills/release/`         | Version bump on a release branch and PR to `main` |

---

## ❓ FAQ

### Can I test GPIO behavior without a Pi?

Not the real pin. Simulation mode covers the HTTP and timing flow; `MockRelay` covers the logic in tests. Verify
wiring on the Pi with the dashboard.

### Why does `cargo build` on my Mac not need `rppal`?

`rppal` is a Linux-only target dependency (`[target.'cfg(target_os = "linux")'.dependencies]`).

### How do I build the ARM binary myself?

Like the release workflow: `cargo install cross`, then `cross build --release --target aarch64-unknown-linux-gnu`.
The binary lands in `target/aarch64-unknown-linux-gnu/release/`.

### My commit was rejected by the `commit-msg` hook. Why?

The message is not a valid Conventional Commit. Use `type(scope): subject`, e.g. `fix(gpio): release pin on drop`.

### Where should a new document go?

In `docs/`, one kebab-case file per domain, linked from the [documentation index](README.md). Agent-only context
stays in `AGENTS.md` or a skill.
