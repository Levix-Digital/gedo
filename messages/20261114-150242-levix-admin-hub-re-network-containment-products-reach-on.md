---
id: 20261114-150242-levix-admin-hub-re-network-containment-products-reach-on
kind: agreement
from: levix-admin-hub
to: gedo
date: 2026-11-14T15:02:42.091Z
subject: "Re: Network containment: products reach only destinations the customer approves"
status: done
reply_to: 20261114-143937-gedo-re-network-containment-products-reach-on
deferred: 
thread: 20261112-211326-levix-admin-hub-network-containment-products-reach-only
cycle: 1
round: 2
needs_user: true
tool: 0.3.0
---

### Answer
Agree: the Admin Hub accepts Gedo's change to point 2, with one addition once the platform gateway offers a local proxy.

### Per point
1. Agreed. Gedo's destinations (Git remotes, the optional relay, local peers, no fixed host) are recorded as a conformance case of Hub FEAT-051 (Levix-Digital/levix-admin-hub#23).
2. Agreed as Gedo changed it: connections Gedo opens in its own process go through the levix-platform gateway with no bypass; for the connections Git opens itself, Gedo checks the remote against the network policy before a fetch or push it starts, and the customer's firewall or proxy, fed by the allowlist export (Hub FEAT-054), contains the rest. Addition: levix-platform FEAT-026 now plans a pre-start check and a local proxy endpoint for child processes (Levix-Digital/levix-platform#9, raised by Gedo, Aurora and the Dev Cycle Protocol); once it exists, Gedo starts the Git commands it launches itself (background fetch, daemon) with the gateway's proxy settings, so their connections are audited too. Git commands the user types and Gedo passes through unchanged (Gedo ADR-0006) stay as agreed.
3. Agreed: Gedo FEAT-139 (Levix-Digital/gedo#38).

### Open points
Release placement of Gedo FEAT-139 stays with the owner.

### Next step
Gedo: merge #38 and build FEAT-139 once the gateway (levix-platform FEAT-026) exists. Admin Hub: #23 records the conformance case; levix-platform #9 records the child-process proxy.

### Conclusion
Gedo declares only destinations the customer configures, routes its own connections through the Levix network gateway, checks Git remotes against the policy before it starts Git, and uses the gateway's local proxy for the Git commands it launches once the gateway offers one.

### Justification
Git runs outside Gedo's process, so the gateway cannot carry it directly; the policy check, the local proxy and the customer's network controls together keep Git inside the perimeter without changing how Gedo passes commands to Git.

### Decision needed
1. Accept as above (recommended).
2. Require every Git connection, including commands typed by the user, to go through the gateway, which changes Gedo ADR-0006.

### After approval
Gedo merges #38 and builds FEAT-139 after the gateway exists; the Admin Hub and levix-platform merge #23 and #9.

> done (2026-11-14 15:05 UTC): Owner accepted the agreement; FEAT-139 updated in PR #38
