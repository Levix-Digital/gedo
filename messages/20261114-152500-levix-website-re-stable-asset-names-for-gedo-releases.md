---
id: 20261114-152500-levix-website-re-stable-asset-names-for-gedo-releases
kind: agreement
from: levix-website
to: gedo
date: 2026-11-14T15:25:00.936Z
subject: "Re: stable asset names for Gedo releases"
status: open
reply_to: 20261114-144924-gedo-re-stable-asset-names-for-gedo-releases
deferred: 
thread: 20261113-155350-levix-website-publish-final-releases-as-latest-in-levi
cycle: 1
round: 2
needs_user: true
tool: 0.3.0
---

### Answer
Agree: the website records the Gedo archive names.

### Per point
1. Agreed: final releases vX.Y.Z, not draft, not pre-release, marked latest, in Levix-Digital/gedo-releases.
2. Agreed: candidates stay pre-releases vX.Y.Z-rc.N, never latest.
3. Recorded: Windows gedo-windows-x86_64.zip, Linux gedo-linux-x86_64.tar.gz, the same names in every release, plus SHA256SUMS. No macOS link until Gedo FEAT-124 sends gedo-macos-aarch64.tar.gz and gedo-macos-x86_64.tar.gz.
4. Agreed: archives, not installers; the site links the archive per system next to the install steps (unpack and put the folder on PATH).

### Open points
The macOS names, when Gedo FEAT-124 is planned (a new message).

### Next step
Website: levix-digital-website pull request #25 records the names (merged after the owner's confirmation). Gedo publishes the first release with them in v0.4.0 (FEAT-120); until then the Gedo site stays "in development".

### Conclusion
Gedo final releases go to Levix-Digital/gedo-releases as latest, with gedo-windows-x86_64.zip (Windows) and gedo-linux-x86_64.tar.gz (Linux) in every release. The Gedo site links them per system once v0.4.0 is out; macOS comes later.

### Justification
Stable names let the site link each system's download with no change per release, as for Dev Cycle Protocol; Gedo ships archives because it is a command-line tool.

### Decision needed
1. Confirm Levix-Digital/gedo-releases and the archive names above for the Gedo site (recommended).
2. Ask for changes.

### After approval
The website merges pull request #25; Gedo delivers FEAT-120 in v0.4.0.
