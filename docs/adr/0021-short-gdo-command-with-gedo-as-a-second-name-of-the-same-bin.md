---
id: 21
title: Short gdo command with gedo as a second name of the same binary
status: accepted
date: '2026-11-09'
version: v0.4.0
subsystems: [surfaces]
supersedes: []
superseded_by: []
amends: [1, 6]
amended_by: []
---

# ADR-0021: Short gdo command with gedo as a second name of the same binary

## Context

ADR-0006 makes `gedo` the developer's only command: every Git command passes through it, so it is typed as often as `git` is today, many times an hour. ADR-0001 names the binary `gedo` after the product. A four-letter command costs a keystroke on every invocation and reads less like the tool it replaces. The project and product name stay Gedo; only the command people type is in question. No CLI exists yet (`gedo-cli` is planned in FEAT-081 for v0.4.0), so the name can change now without breaking any user, script or installed hook.

A short command must not collide with what developers already have. On 2026-11-08 the three-letter candidates were checked against crates.io, npm, Homebrew formulae and casks, Scoop (Main and Extras), winget package identifiers, the binaries of Debian stable and Ubuntu 24.04 packages, the aliases of the oh-my-zsh and prezto plugins, and the default PowerShell aliases:

| Name | Result |
|---|---|
| `gdo` | Free everywhere; an npm library named `gdo` exists but installs no command |
| `ged` | Free everywhere |
| `gdx` | An active npm command-line tool for Git ("Git, but with better DX") and a prezto alias |
| `gdu` | A widely packaged disk usage tool (apt, Homebrew, Scoop, winget) |
| `gdr` | A default PowerShell alias (`Get-PSDrive`) |
| `edo` | The crate name is taken on crates.io |

## Decision

- **People type `gdo`.** The CLI binary is installed as `gdo`, three letters like `git`. Documentation, examples and help text use `gdo <command>`.
- **`gedo` is a second name of the same binary.** Every release archive and package (cargo install, winget, Linux packages) installs both names: a link on Linux and macOS, a hard link or second copy on Windows. Both names behave identically, take the same arguments and report the same build in `version`; neither name changes behavior.
- **Machines call `gedo`.** The Git hooks installed by `init`, the merge driver configured in Git (`gedo merge-driver`) and Gedo's own scripts call the long name, which is unambiguous in automation and stable if the short name ever has to change.
- **Everything else keeps the product name.** The project, the crates (`gedo-cli`, `gedo-app` and the others), the daemon `gedod`, the references `refs/gedo/*`, the local data in `.git/gedo/`, the environment variables `GEDO_*`, the reserved label prefix `gedo.` and the hash and id contexts (`gedo ... v1`) are unchanged: they are formats and identifiers, not commands, and they freeze with v1.0.0.
- This amends ADR-0001 (the binary of `gedo-cli` is `gdo`, with `gedo` as its second name) and ADR-0006 (the dispatcher, the pass-through and the hooks apply to both names; examples written `gedo <command>` there read `gdo <command>`).

## Alternatives Considered

- **Keep only `gedo`**: no extra name to install, but a longer command typed all day for a tool meant to replace `git`.
- **Rename the product to three letters**: the name Gedo was already decided and is the one to register as a trademark (ADJ-001).
- **Only `gdo`, no second name**: one name less to install, but hooks and scripts would depend on the short name, the one most likely to collide with something on a user's machine.
- **`ged`**: also free, but reads less like Gedo and every letter is typed with the same hand.
- **A shell alias documented for users**: no install work, but it does not exist in hooks, IDEs, CI runners or Windows shells unless each user sets it up.

## Consequences

- The CLI skeleton (FEAT-081) and the release pipeline (FEAT-120), followed by the packages (FEAT-122, FEAT-123), install both names; ADJ-002 tracks it in v0.4.0.
- `doctor` (FEAT-086) can report when only one of the names is on the `PATH`, or when the two resolve to different builds.
- The end-to-end suite (FEAT-092) runs its scenarios through `gdo` and checks that `gedo` gives the same result.
- A future collision with `gdo` costs only the short name: hooks, the merge driver and scripts keep working through `gedo`.
- Verification: after installing a release on Windows and Linux, `gdo version` and `gedo version` print the same build, and `gdo init` writes hooks that call `gedo`.
