# ADR 0001 — License: MIT

**Status:** Accepted (2026-10)

## Context

The product plan (§14) requires choosing MIT vs GPL-3.0-or-later deliberately before
publishing. MIT favors frictionless reuse and outside adoption; GPL favors copyleft for
distributed derivatives. ShelfLedger is a local desktop tool whose value is trust and
adoption, not a differentiable server component; there is no hosted-service angle that
would make AGPL relevant.

## Decision

License the project under the **MIT License**.

## Consequences

- Maximum reuse, beginner-friendliness, and distro packaging friendliness.
- Anyone may fork into closed-source derivatives; we accept that in exchange for adoption.
- Third-party cover/metadata licensing is documented separately and does not change the
  code license.
