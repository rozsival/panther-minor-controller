---
name: release
description: >
  Bump version, commit, tag, and push to remote. Use when user says "release" or "bump version".
---

You are the release engineer for Panther Minor Controller. Follow this workflow precisely.

## Prerequisites

1. **Check branch** — run `git rev-parse --abbrev-ref HEAD`. Must be `main`.
   - If not on `main`, abort and tell the user to switch to `main` first.
2. **Check for uncommitted changes** — run `git status --porcelain`. Must be empty.
   - If dirty, abort and ask the user to commit or stash changes first.
3. **Pull latest** — run `git pull --rebase` to ensure you're up to date.

## Version Bump

Ask the user what type of release this is (major, minor, patch). Prefer tool to ask question if available, otherwise ask in the chat.

Wait for their answer. Then bump the version using semver:

| Type  | Current `X.Y.Z` | New `X.Y.Z` |
| ----- | --------------- | ----------- |
| major | `X.Y.Z`         | `X+1.0.0`   |
| minor | `X.Y.Z`         | `X.Y+1.0`   |
| patch | `X.Y.Z`         | `X.Y.Z+1`   |

The version lives in exactly three authored files:

| File           | Occurrence                                                                                                           |
| -------------- | -------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`   | `version = "X.Y.Z"` under `[package]` — not the pinned dependency versions                                           |
| `package.json` | `"version": "X.Y.Z"`                                                                                                 |
| `README.md`    | `vX.Y.Z` in three `wget` release URLs — "Set up the Raspberry Pi", "Install the controller", "Update the controller" |

`Cargo.lock` also carries it, at `name = "panther-minor-controller"` / `version = "X.Y.Z"`. It is **not**
in that table on purpose.

> **Never hand-edit `Cargo.lock`.** It is generated, and `cargo build` in the next section rewrites that
> entry from `Cargo.toml`. Editing a lockfile by hand is always wrong. The same goes for anything under
> `target/`.

Do **not** trust the table blindly — file locations drift. Before editing, discover

```bash
grep -rn "<CURRENT_VERSION>" --include=*.toml --include=*.json --include=*.md . \
  | grep -vE "node_modules|/\.git/|target/|lock"
```

Update each match in-place. Read **only the matching line ranges** (e.g. `README.md:95-105`) if your edit
tool needs a fresh read to anchor a hunk — never read a whole file to change one line.

## Commit & Tag

1. **Refresh lockfiles** after version bump:
   ```bash
   pnpm install
   cargo build --workspace
   ```
2. **Verify no stale version remains** — grep for the **old** version now that `Cargo.lock` has been
   regenerated. This must print nothing:
   ```bash
   grep -rn "<OLD_VERSION>" --include=*.toml --include=*.json --include=*.md --include=Cargo.lock . \
     | grep -vE "node_modules|/\.git/|target/|pnpm-lock"
   ```
   If anything prints, fix it before continuing. A missed `README.md` `wget` URL ships a release whose
   own install instructions point at the previous version.
3. **Gather all changed files**:
   ```bash
   git add $(git diff --name-only HEAD)
   ```
4. **Commit**:
   ```bash
   git commit -m "chore(release): vX.Y.Z"
   ```
5. **Create a signed tag**, then confirm it exists. Run it as its own command — hook output on stderr
   (commitlint via lefthook) can make a chained `git commit && git tag -s …` look like a failure and
   abort before the tag is created, leaving a release commit with no tag:
   ```bash
   git tag -s vX.Y.Z -m "Release vX.Y.Z"
   git tag --list vX.Y.Z
   ```
   If the second command prints nothing, the tag was **not** created. Stop and report the error — do not
   push a release commit without its tag.
6. **Push to remote** — push **sequentially**, NOT in parallel:
   ```bash
   git push origin vX.Y.Z
   git push origin main
   ```
   Push the tag first, then the branch. Running both pushes concurrently can cause the tag to be pushed twice (resulting in "reference already exists") and the branch push to fail.
   If `git push origin main` is rejected due to required status checks, wait for checks to complete and retry once. Do not retry more than once.

## Confirmation

Report back to the user:

```
✅ Release vX.Y.Z created successfully.
   - Version bumped in: {list all files that were modified during the release}
   - Committed: chore(release): vX.Y.Z
   - Tagged: vX.Y.Z
   - Pushed to remote
```

## Error Handling

- If the version format is unexpected, abort and ask the user to verify it follows `X.Y.Z` semver or is approved to be in a different format (e.g., `X.Y.Z-beta`).
- If `git push` fails (e.g., remote rejects tag, network issue), inform the user and stop. Do not retry automatically.
- Never auto-approve — always confirm each step with the user before proceeding when the action is irreversible (push to remote).
