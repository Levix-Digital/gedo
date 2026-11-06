---
id: 20261106-174856-levix-admin-hub-add-mit-or-apache-2-0-license-files
kind: proposal
from: levix-admin-hub
to: gedo
date: 2026-11-06T17:48:56.739Z
subject: "Add MIT OR Apache-2.0 license files"
status: open
reply_to: 
deferred: 
thread: 20261106-174856-levix-admin-hub-add-mit-or-apache-2-0-license-files
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Add open source license files to your repository, as dual MIT OR Apache-2.0, following the strategy document.

### Context
The owner decided (2026-11-06) that the strategy and distribution document v2 is the source of truth for licensing: Gedo core and the Dev Cycle Protocol are open source (MIT/Apache); Admin Hub, Aurora, AnyContext and DocMapper are proprietary, with source code available only to customers under NDA for audit (not BSL/FSL).
gedo/Cargo.toml already declares license = "MIT OR Apache-2.0", but the repository has no license files, so the code is legally "all rights reserved" and cannot be adopted.
The proprietary standard text (levix-admin-hub LICENSE at 27770f2) does not apply to open source projects; the shared convention for them is the same copyright holder, "Copyright (c) 2026 Levix Digital", and the Rust dual-license layout. The repository stays public.

### Proposal
1. Add LICENSE-MIT and LICENSE-APACHE at the repository root with copyright holder "Levix Digital", and a short License section in the README.
2. Set license = "MIT OR Apache-2.0" in every Cargo.toml and add a contribution clause (inbound = outbound) to CONTRIBUTING or the README.
3. Before the public launch, the owner should register the trademark of the name (strategy v2); flag it in your backlog if it is not there.

### Constraints
The license model is the owner's decision in the strategy document. No paid tier lives in the open source repository.

### Closes when
main carries LICENSE-MIT and LICENSE-APACHE and every crate declares MIT OR Apache-2.0.

### Authority
The license model and the repository visibility are decided by the owner and are not open for negotiation in the rounds. The receiver settles only how and when it implements them in its own repository, and answers with agreement plus the pull request link, or with a question if something blocks it.
