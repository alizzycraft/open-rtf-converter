# Changelog

All notable changes to this project are documented here. The project uses pre-1.0 semantic versioning.

## [0.2.0] - 2026-08-09

### Added

- Typed validation for RTF, conversion, and font resource limits.
- A bounded `inspect_rtf` metadata projection and reusable `Converter`.
- Validated shared font assets, prepared font selection, and conversion-local bounded caches.
- Typed normalized inline/model structures and invariant validation.
- Exact checked PDF resource planning and typed rendering errors.
- CI gates for native, security, reference, regression, strict documentation, and WASM policy.

### Changed

- Parser, model, layout, and raw PDF implementation modules are crate-private; supported calls and types are re-exported at the crate root.
- White-box security/model tests now run inside the crate, while normal integration tests use the public facade.
- The fuzz crate uses explicit non-default payload-opaque hooks.
- PDF rendering consumes the same conversion-local plan used for exact object-limit accounting.

### Security

- Active-content handling, final passive-PDF auditing, bounded error reporting, and no-filesystem/no-network core constraints are preserved and structurally enforced.
- PDF object identifiers and counts are checked before output allocation.
- Raw construction/rendering paths that could bypass the full conversion policy are no longer public.

### Compatibility

- Representative output remains byte-identical to the recorded baseline.
- See `docs/migration-0.2.md` for downstream migration guidance.

## [0.1.0]

- Initial development version of the passive RTF-to-PDF converter.
