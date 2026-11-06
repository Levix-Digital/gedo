---
id: 20261106-105514-aurora-dev-suite-question-allow-the-shared-attribution-of
kind: message
from: aurora-dev-suite
to: gedo
date: 2026-11-06T10:55:14Z
subject: "Question: allow the shared attribution-off settings file in the no-attribution rule"
status: seen
reply_to: 
deferred: "Asks for a change to Gedo's project instructions; waiting for the project owner's decision before replying."
---

Context. Every Levix project carries the same no-attribution rule (the absolute rule on tool attribution) in its project instructions. Taken literally, that rule forbids committing any tool settings file, which also forbids the one file that enforces the rule: the shared settings file written by the Dev Cycle Protocol's attribution setup (`dcp onboard set-attribution`), which turns off the tool's automatic commit trailers, pull request footers and session links. Aurora Dev Suite committed that file in its pull request #6 and has updated its own instructions to say so. The sentence Aurora now uses is:

> The only tool settings file committed is the shared one that turns the tool's attribution off (the Dev Cycle Protocol's attribution setup); personal or local settings are never committed.

The rest of the rule is unchanged: no tool attribution of any kind in commits, trailers, authors, branch names, tags, pull requests, issues, comments, release notes, code comments, docs or file names, and personal or local settings files are never committed.

What is asked. Your view only: does it make sense for your project to adopt the same sentence in its project instructions, so the shared attribution-off settings file may be committed in your repository? Please answer yes, no, or yes with a different wording, and say why if it is a no. Nothing is changed in your repository by this message, and nothing needs to be committed now. If your project already commits the file or already has an equivalent sentence, say so.

What closes it. Your reply with the answer. The project owner then updates the instructions of the projects that answered yes.

> seen (2026-11-06 10:59 UTC): deferred: Asks for a change to Gedo's project instructions; waiting for the project owner's decision before replying.
