# Branch protection for `main`

`main` only takes changes through pull requests whose CI is green (SC-008). The repository owner enables this once, in **Settings → Rules → Rulesets** (or **Settings → Branches → Branch protection rules**), for the branch `main`.

## Settings

- Require a pull request before merging.
- Require status checks to pass before merging, with the branch up to date with `main`.
- Block force pushes and deletions.

## Required status checks

These are the job names of `.github/workflows/ci.yml`, exactly as GitHub lists them:

| Check | What it covers |
|---|---|
| `test (ubuntu-latest)` | Format, Clippy, all tests, the SSH remote test and `cargo xtask arch-check` on Linux |
| `test (windows-latest)` | Format, Clippy, all tests and `cargo xtask arch-check` on Windows |
| `msrv (1.95)` | The workspace builds with the minimum supported Rust version |
| `deny` | `cargo deny check`: licenses, duplicate versions, sources and RustSec advisories |

The `bench (...)` jobs are informational and are not required. The `host-validation` workflow runs on a schedule and by hand, not on pull requests, so it is not a required check either.

GitHub only offers a check in the list after it has run once on the repository, so the first CI run must finish before the checks can be selected.

## Verifying

Open a draft pull request with a deliberate failure (for example a `cargo fmt` violation). The merge button must stay blocked until the failure is fixed; close the pull request afterwards.
