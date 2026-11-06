# Termrock

Termrock is a Rust terminal UI component library with four
deterministic demonstration applications: Showcase, TablePro, Holla
and Jackin. The library owns interaction and state; the applications
prove the components compose in realistic screens.

## Generated-workflow snapshot bounds

The generated-workflow guard accepts recursive Git tree listings up to 8 MiB and 8,192 entries, with a 16 MiB maximum per blob and a 512 MiB maximum total of materialized blob occurrences. The measured baseline HEAD contains 608 blobs totaling 191,075,553 raw Git blob bytes; its largest blob is 1,876,744 bytes. The limits apply to raw blob bytes, count repeated object IDs and symlink targets once per tree entry, and leave room above that measured snapshot. Future or historical snapshots beyond these limits are rejected.

When a validated selected snapshot is missing blobs but its commit and tree metadata are available, the guard retrieves only those blob IDs from the fixed official GitHub blob API and verifies the declared identity, decoded bytes, and Git object hash before publishing them to a private verified-blob store. The trusted-seed reader and selected-snapshot reader share this blob-only store while keeping their private tree-object stores separate. Across one checker invocation, hydration is bounded to 1,024 missing objects, 4 MiB per blob, 256 MiB total decoded bytes, and 15 minutes. Repeated pre-push records reuse already verified objects and receive separate bounded response scratch directories. The API must have each object, so unpublished local-only blobs remain unavailable. Missing commits and trees, and missing blobs in SHA-256 repositories, fail closed. These hydration limits are narrower than the 16 MiB and 512 MiB materialization limits above.

## Documentation

- [Current state](docs/current-state.md) — architecture and
  structure as they exist today.
- [Future architecture](docs/architecture.md) — the refactoring
  target: crate structure, ownership contracts, migration gates.
- [Product contract](docs/product.md) — visual and interaction
  authority.
- [Requirements](docs/requirements.md) — conceptual acceptance
  surface per app and journey.
- [Functional testing](docs/functional-testing.md) — the single
  test estate and its gates.
- [Development](docs/development.md) · [CI](docs/ci.md) ·
  [Dependencies](docs/dependencies.md) · [Decisions](docs/decisions.md)

Agent working rules are in `AGENTS.md` (`CLAUDE.md` is a symlink to
it).
