# [Product name]: Technical Specification

[One paragraph: the technical shape of the product (for example "a mobile app with a REST backend and a web admin") and the architecture style chosen (`architecture.style` in `.dev-cycle.json`). How each part works is described in the technical documentation (TECDOC.md); this document holds the product-wide technical decisions and requirements.]

## Platforms and Stack

[Target platforms and versions; languages, frameworks, databases and main libraries, each with one line on why. Founding choices with alternatives also get an ADR.]

## Architecture Overview

[The main parts and how they talk to each other, in a few sentences or a small diagram; link to TECDOC.md for the details.]

## Non-Functional Requirements

[Measurable requirements, each one testable. Use the subsections that apply and write "Not applicable" with a reason for the others.]

### Performance

[For example: p95 response time under 300 ms for 1,000 concurrent users.]

### Security and Privacy

[Authentication, authorization, data protection, personal data and the regulations that apply.]

### Availability and Recovery

[Uptime target, backups, recovery time.]

### Accessibility and Localization

[Accessibility standard, languages and regions.]

## Integrations

[External systems, APIs and services the product depends on or exposes, with the direction of the data and what happens when each is unavailable.]

## Data

[The main entities and their relations, where data lives, retention, migrations policy (additive or backward compatible).]

## Constraints

[Hard limits the design must respect: budget, hosting, licenses, team skills, deadlines, existing systems. These become principles of the constitution when Spec Kit is used.]

## Open Questions

None.
