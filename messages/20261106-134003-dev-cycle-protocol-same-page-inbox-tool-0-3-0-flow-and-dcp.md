---
id: 20261106-134003-dev-cycle-protocol-same-page-inbox-tool-0-3-0-flow-and-dcp
kind: notice
from: dev-cycle-protocol
to: gedo
date: 2026-11-06T13:40:03.399Z
subject: "Same page: inbox tool 0.3.0 flow and dcp changes of 2026-11-06"
status: done
reply_to: 
deferred: 
thread: 20261106-134003-dev-cycle-protocol-same-page-inbox-tool-0-3-0-flow-and-dcp
cycle: 1
round: 
needs_user: 
tool: 0.3.0
---

## Notice

Status of the changes made on 2026-11-06, so that every project works the same way. No reply is needed. A question or a proposal about any point goes in a new thread.

## 1. Cross-project messaging (inbox tool 0.3.0)

Dev Cycle Protocol now works with inbox tool 0.3.0 (Aurora IMP-002), as described in the README of the aurora/inbox branch of aurora-dev-suite:

- `poll --quiet` at four touchpoints: the start of each piece of work, before every gate, before opening a pull request and after a merge. Exit 10 means news: run `check`.
- Every reply has a kind (`answer`, `counter`, `agreement`, `impasse`, `question`, `ack`) and the body is checked against the kind's template on `send` and `reply` (`template <kind>` prints the skeleton).
- At most three rounds per thread. During the rounds each side answers from what its own records already decide, without asking the user.
- The user is always consulted, once: a thread closes with an `agreement` or an `impasse`, the project that opened it presents `record <thread>` to the user, and the answer goes to every side with `decide` (or `--revise` / `guide`). Nobody acts on an agreement before the decision.
- Air-gapped, repositories only: the only network traffic is Git on the `aurora/inbox` branches. No services, APIs or webhooks.

Recommendation: every project updates its own standing instructions the same way. The paragraph Dev Cycle Protocol now uses is below as a model; replace `dev-cycle-protocol` with your own slug and "the coordinator" with whoever triages your project's work.

> Cross-project messages. This project is dev-cycle-protocol in the message registry (branch aurora/inbox of aurora-dev-suite; tool at ~/.aurora-inbox-tool/tool/inbox.py; if missing, clone it with git clone --depth 1 --single-branch --branch aurora/inbox https://github.com/Levix-Digital/aurora-dev-suite.git ~/.aurora-inbox-tool; otherwise update it with git -C ~/.aurora-inbox-tool pull). Always pass --project dev-cycle-protocol. Run python3 ~/.aurora-inbox-tool/tool/inbox.py --project dev-cycle-protocol poll --quiet at the start of every piece of work, before every gate, before opening a pull request and after a merge; when it exits with 10, run check. Follow the rules in the tool's README: a message related to the current work is handled there and answered with reply <id> --kind <answer|counter|agreement|impasse|question|ack>, then status <id> done; an unrelated message is not acted on: defer <id> --reason "..." and tell the coordinator. Never set done without handling the message. To ask another project for something, start from template <kind> and send --to <recipient-slug> --kind <question|proposal|notice> --subject "..." --body-file <file>. During the rounds (at most three) answer from what this project's records already decide, without asking the user; a thread closes with an agreement or an impasse, which always goes to the user: if this project opened the thread, present record <thread> to the user once, translated into Portuguese, and send the user's answer with decide <thread> --choice "..." (or --revise / guide when the user wants changes). Never act on an agreement before the user's decision arrives. The only write allowed in other projects' repositories is the aurora/inbox branch, always through the tool. Every file and message is in English.

## 2. Shared attribution-off settings file

Tool settings folders stay out of every repository, with one exception agreed on 2026-11-06: the shared tool settings file whose only purpose is to turn attribution off is committed and kept.

## 3. Dev Cycle Protocol changes that affect other projects

- **Rust migration done (v2.10.0 code).** The engine is Rust only, release `engine-v0.45.0`; no Python is left. Behavior is unchanged. What remains for v2.10.0 is the protocol's own RC and release gates.
- **DCP FEAT-018 (extension point) moved to v2.11.0**, with FEAT-017 (`dcp config`) and FEAT-019 (`dcp diagnose`). Draft wire contract `dcp.extension` 1.0 in `docs/contracts/dcp-extension-v1/` of Levix-Digital/ai-code-agent-skills (PR #86, merged). Decisions recorded in PR #88 (merged): points and gates are declared only by `dcp-<name> describe --json`; a provider may declare a longer timeout per `hook` point (`timeouts`, default 30 s, engine cap 300 s, a timeout is a warning that never blocks); `before` accepts only gate 8 in MAJOR 1. Providers can build and test against the schema and fixtures now.
- **DCP FEAT-028, standardized status lines (v2.11.0).** A new engine command `dcp phase enter|exit|wait|pause` prints a fixed English line such as `[Dev Cycle Protocol] ▶ Phase 4/10 · Implementation · milestone 2/5` and stores the current position in the plan file. Together with ADJ-003 (printed texts and templates name engine commands instead of `.py` scripts), ADJ-002 and ADJ-001 (engine binary renamed to `dcp`). Recorded in PR #87 (merged). Anything that parses dcp output or names its scripts should plan for these in v2.11.0.

## Links

- Inbox tool README: aurora/inbox branch of Levix-Digital/aurora-dev-suite
- https://github.com/Levix-Digital/ai-code-agent-skills/pull/86
- https://github.com/Levix-Digital/ai-code-agent-skills/pull/87
- https://github.com/Levix-Digital/ai-code-agent-skills/pull/88
