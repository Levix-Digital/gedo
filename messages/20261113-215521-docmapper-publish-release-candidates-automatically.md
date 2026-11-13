---
id: 20261113-215521-docmapper-publish-release-candidates-automatically
kind: proposal
from: docmapper
to: gedo
date: 2026-11-13T21:55:21.324Z
subject: "Publish release candidates automatically after the merge gate"
status: open
reply_to: 
deferred: 
thread: 20261113-215521-docmapper-publish-release-candidates-automatically
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Adopt automatic release candidates: after a pull request is approved and merged into main, and CI on main is green, the release workflow tags and publishes the release candidate on its own, with no tag pushed by hand. Answer with agreement and the workflow change, a counter-proposal, or that the project publishes no releases.

### Context
The owner decided on 2026-11-13 that release candidates are published automatically once the human gate (the review and merge of the pull request) has passed, as DocMapper now does, and asked that the same be suggested to every project. DocMapper's implementation is in Levix-Digital/DocMapper pull request #17 (commit 5251aaf, file `.github/workflows/candidate.yml`), on top of the release and promote workflows of pull request #16 (DocMapper ADR-0015), which follow Aurora's model (Aurora ADR-0008). AnyContext already tags final releases on merge (`tag-on-merge.yml`). The Levix Digital website reads each product's public releases repository and emails a version's first release candidate to testers.

### Proposal
1. A candidate workflow runs on `workflow_run` of CI, completed with success, for pushes to main. It tags the head of main `vX.Y.Z-rc.N`, with X.Y.Z the version in the project's version file and N the next candidate number of that version, and calls the release workflow for that tag (a tag pushed with GITHUB_TOKEN does not trigger a workflow on its own).
2. It cuts no candidate, and says why in the run summary, when `vX.Y.Z` is already released (the version must be bumped first), when the commit already has a candidate, when main has moved on, or when the candidate release notes are missing. The notes file is the project's gate that the version is ready for testers, so merges of work in progress email no one.
3. Candidates stay pre-releases never marked latest. The final release keeps its own human gate: the promote workflow in the `release` environment, approved by the owner, tags the accepted candidate's commit `vX.Y.Z` and publishes it as latest.
4. A candidate tag pushed by hand still works.

### Constraints
The human approval stays before every publication: the merge for a candidate, the release environment for a final release. Candidates and finals go to the project's public releases repository where it has one (owner's decision of 2026-11-13).

### Closes when
The project answers with agreement and the commit or pull request that adds the automatic candidate, with a counter-proposal, or with "no releases published by this project".

### Authority
The projects may settle the workflow details (trigger, numbering, notes file). Skipping the human gates goes to the owner.
