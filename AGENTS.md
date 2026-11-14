# Gedo

This file is the single source of truth for any coding agent working here. `CLAUDE.md` only imports it. Tool-specific features (hooks, subagents, JSON agents) are a convenience and never a dependency: everything an agent needs is in this file and in `.agents/`.

## 1. What this repository is

Gedo (Git Event-Driven Overlay) is a collaboration overlay built on Git's object store: a team collaborates on code entities (functions, types, modules) instead of lines of text, with conflicts merged by meaning, collisions found while they are born and history per entity, without a mandatory server and without breaking any Git tool. It is an open source Rust workspace (MIT OR Apache-2.0) in hexagonal architecture: a pure core, adapters for gitoxide, Tree-sitter, redb and the network, and the `gdo` command (also installed as `gedo`) and the `gedod` daemon as surfaces. The vision is in `docs/product/VISION.md`, the technical constraints in `docs/product/TECH_SPEC.md` and the system overview in `TECDOC.md`.

Map:

| Path | What lives there |
| --- | --- |
| `AGENTS.md` | Instructions for agents (this file) |
| `.agents/skills/` | Project skills, one folder per skill with a `SKILL.md` (the Spec Kit skills live here) |
| `.agents/memory/` | What past sessions learned (see section 4) |
| `docs/adr/` | Decision records (ADR format) |
| `docs/architecture/` | One page per subsystem (core, store, parsing, identity, events, graph, net, surfaces) |
| `docs/product/` | `VISION.md`, `TECH_SPEC.md` and `UX_REFERENCE.md` |
| `docs/BACKLOG.md`, `docs/ROADMAP.md` | Work items and releases |
| `docs/plans/` | Plans of the Dev Cycle Protocol, one per feature or bug fix |
| `docs/ci/` | Branch protection and CI notes |
| `specs/` | Spec Kit specifications, one folder per feature |
| `crates/core/` | The core crates (domain model, identity, events, graph, use cases); no adapter dependencies |
| `crates/adapters/` | The store, parsing and network adapters and the surfaces |
| `tools/xtask/` | Repository checks (`cargo xtask arch-check`) |
| `fuzz/` | Fuzz targets (nightly, `cargo-fuzz`) |
| `tests/MANUAL_TESTS.md` | Manual tests |
| `.github/workflows/` | CI, fuzzing and host validation |
| `.specify/` | Spec Kit configuration and the generated constitution (`.specify/memory/constitution.md`) |
| `.dev-cycle.json` | Dev Cycle Protocol configuration |
| `.aurora/integrations.json` | Registry of the other Levix projects for cross-project messages |

Skills are in `.agents/skills/`. If your tool does not list them, read the skill's `SKILL.md` from there. `.claude/skills` is a versioned relative link to `../.agents/skills`. If it is a plain file instead of a link (a Windows clone without symlink support), tell the person and show the fix:

```
# once per machine: turn on Developer Mode in Windows settings, then
git config --global core.symlinks true
# in this clone
del .claude\skills
git checkout -- .claude/skills
```

Do not replace the link with a junction: it leaves `git status` dirty.

## 2. How to work

This repository uses the Dev Cycle Protocol (DCP). Start every piece of work with `/dev-cycle`, using the latest version of the protocol (update its clone to the latest `main` first), and follow the stage it puts you in. Follow every phase strictly, with the protocol's own commands (the documentation and README checkers, the constitution generator and checker, the backlog, roadmap and milestone tools, the benchmarks): run them yourself, never hand them to the owner. If the protocol is not installed (no `/dev-cycle` skill), stop and install it first:

```
git clone https://github.com/Levix-Digital/dev-cycle-protocol.git ~/.ai-code-agent-skills
~/.ai-code-agent-skills/install.sh            # Windows: & "$HOME\.ai-code-agent-skills\install.ps1"
```

If the skill still does not load, read `~/.ai-code-agent-skills/dev-cycle/SKILL.md` and follow it. Decisions go in `docs/adr/`; the constitution in `.specify/memory/constitution.md` is generated, never edited by hand (run the protocol's constitution generator after changing an ADR, the technical specification or `.dev-cycle.json`).

The owner's gates are plan approval, release candidate acceptance and the release itself; never take them for the owner. The session opens the pull request with its full description and follows it through CI; the owner merges. The protocol's review of a pull request is done right after opening it and given to the owner in the conversation, never posted on GitHub.

### Cross-project messages

This project is `gedo` in `.aurora/integrations.json`. Each project's messages travel on its own repository's `aurora/inbox` branch, read and written with `dcp-integrations` and the session's own Git credentials (installed from the public `Levix-Digital/aurora-releases` repository; `dcp-integrations version` checks it is there; if it is missing, say so instead of using another tool). Run it from the repository root.

- Run `dcp-integrations check` at the start of work: it lists every message still pending, deferred ones included. Run `poll` before every gate, before opening a pull request and after a merge; when it reports news, run `check`.
- A message related to the current work: handle it, answer with `reply <id> --kind <answer|counter|agreement|impasse|question|ack> --body-file <file>`, then `status <id> done`. Answer from what the project's records already decide, without asking the owner mid-conversation.
- A message unrelated to the current work: do not act on it; run `defer <id> --reason "..."` so it stays pending, and list it for the owner at the end of the session. A later session that finds it at `check` decides whether it now belongs to its work. Never set `done` without handling the message.
- To ask another project for something: write the body from `template <question|proposal>` and run `send --to <recipient-slug> --kind <question|proposal> --subject "..." --body-file <file>`.
- A conversation has at most three rounds and closes with an agreement or an impasse. The project that opened it presents the closing record to the owner once (`record <thread>`) and sends the owner's answer with `decide <thread> --choice "..."`. No project acts on an agreement before that decision.
- Every message is in English.

## 3. Repository rules

- **Ownership**: Levi Guilherme (LeviGuilherme) owns this repository and decides what merges. Ask before anything that cannot be undone.
- **Repositories this project writes to**: its own, `Levix-Digital/gedo`; the shared Levix library `Levix-Digital/levix-platform`; and `Levix-Digital/gedo-releases`, only to publish Gedo's releases. Only in these does work from here create branches, commit, push, open or update pull requests, comment, tag, release or change settings. `levix-platform` is a store of reusable pieces with no owner project: every change there goes through a pull request, stays generic (nothing specific to one project) and must not break the projects that depend on them; the project that needs the change opens and reviews the pull request. Every other repository is read-only, the Dev Cycle Protocol's included (a change the protocol needs is asked for with a message to `dev-cycle-protocol`). `Levix-Digital/levix-admin-hub` is a product with its own project, read-only for Gedo: a change Gedo needs there is asked for with a message to `levix-admin-hub`. The only exception is another project's `aurora/inbox` branch, always through `dcp-integrations`. A change another project needs in its own repository is asked for with a message, never made there. If a task seems to need any other write, stop and ask the owner.
- **Branches**: `feat/NNN-slug` for a Spec Kit feature, otherwise `<type>/<slug>` with a Conventional Commit type (`fix/...`, `docs/...`, `chore/...`); `levix/<slug>` when no type fits. Never use a `claude/` prefix or any other tool name.
- **Commits**: Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:` ...), in English.
- **Git identity**: commit as the person you are working for, with the name and email already configured in Git. Never set an agent or tool identity. In an environment whose Git identity is a tool's default, set `git config user.name LeviGuilherme` and `git config user.email levi.silveira@levix.digital` before the first commit.
- **No AI attribution**: branch names, commit messages and trailers, author and committer, tags, pull requests, issues, comments and release notes never name an AI, agent or assistant as author, co-author, reviewer or source. Agent configuration folders and files (AGENTS.md, CLAUDE.md, .agents/, .claude/, .specify/) are allowed.
  - Some tools append a footer or a link on their own: after creating or editing a pull request, issue or comment, read it back and remove anything appended. Do not post comments or reviews on GitHub; report in the conversation instead. Before every push, check the commits and the diff for such text. If something already pushed breaks the rule, fix it where possible and tell the owner what remains.
  - `.claude/settings.json` exists only to turn automatic attribution off and stays committed; other tool settings folders and personal or local settings are never committed (exclude them in `.git/info/exclude`).
- **Language**: every file in the repository is in English.

## 4. Memory

`.agents/memory/` holds exactly four files. Never add others.

| File | Holds |
| --- | --- |
| `preferences.md` | How the owner wants to work |
| `workflow.md` | How work flows; tool pitfalls |
| `environment.md` | Machines, cloud, CI: versions, blocked hosts |
| `product.md` | Product directions not yet in vision docs or decisions |

- **Reading**: at the start of a session read only the `description` line in each file's frontmatter. Open a file only when it touches the task.
- **Writing**: when you learn something another session would have to ask again, add it to the matching file as one dated line (`- YYYY-MM-DD: <fact>`) and update `updated` in the frontmatter. Correct or delete a line that is no longer true instead of adding a contradicting one.
- **Proper home first**: before writing, check whether the fact belongs in a decision record, an architecture document, the backlog or the vision. If it does, put it there and not in memory.
- **Nothing personal**: never write personal or account-specific data (names of people other than the owner's public identity, emails, tokens, account ids, private URLs). This repository may be public.
- Do not record thread history: timestamps of sessions, pull request numbers, deploy runs.

## 5. Everyday commands

| Task | Command |
| --- | --- |
| Install tools | `rustup toolchain install` (Rust 1.95 or later) and `cargo install cargo-deny --locked` |
| Build | `cargo build --workspace` |
| Test | `cargo test --workspace --all-features` |
| Format | `cargo fmt --all --check` |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| Architecture and dependency checks | `cargo xtask arch-check`, `cargo deny check` |
| Benchmarks | `cargo bench -p <crate> --bench <name>` (budgets are measured on the reference machine, see `environment.md`) |
| Fuzzing | `cargo +nightly fuzz run <target>` (needs nightly and `cargo-fuzz`) |
