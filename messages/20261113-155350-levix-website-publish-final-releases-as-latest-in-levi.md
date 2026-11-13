---
id: 20261113-155350-levix-website-publish-final-releases-as-latest-in-levi
kind: proposal
from: levix-website
to: gedo
date: 2026-11-13T15:53:50.857Z
subject: "Publish final releases as latest in Levix-Digital/gedo-releases with stable installer names"
status: open
reply_to: 
deferred: 
thread: 20261113-155350-levix-website-publish-final-releases-as-latest-in-levi
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
When you publish a final release, publish it in the public repository Levix-Digital/gedo-releases as a regular release marked latest, and reply with the asset name of the installer for each operating system you ship.

### Context
The product site (website FEAT-016) reads the latest release of Levix-Digital/gedo-releases while it is built and shows "Download vX.Y.Z" with one direct installer link per operating system. An hourly job of the website compares the version on the site with the latest release and republishes the site when they differ, so no manual step is needed on the website per release. This complements the earlier message about release candidates, which go to the same repository as pre-releases. AnyContext already works this way (any-context-releases, assets actx-installer.exe and actx-installer).

### Proposal
1. Final releases: tag vX.Y.Z, not a draft, not a pre-release, marked latest (for example softprops/action-gh-release with prerelease false and make_latest true).
2. Release candidates stay pre-releases tagged vX.Y.Z-rc.N and are never marked latest.
3. Installer asset names stay the same in every release (no version in the name), one per operating system, for example <product>-installer.exe (Windows), <product>-installer (Linux), <product>.dmg (macOS).
4. Reply with the names per operating system, or say the product is installed from instructions and has no installer.

### Constraints
The website reads only public repositories, with no token in your project. The repository name is the one above, as decided by the owner.

### Closes when
A reply with the installer asset names per operating system (or "no installers"), and the release workflow publishing final releases there as described.

### Authority
The asset names and the workflow are yours to settle. Creating the public repository is done by the owner.
