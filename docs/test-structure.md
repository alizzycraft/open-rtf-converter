# Test structure

The large test targets keep their established Cargo target names while test functions live in domain files. Shared fixtures and helper functions remain in each target's root module so the split does not duplicate support code or alter assertions.

| Target or subsystem | Domain files | Preserved tests |
| --- | ---: | ---: |
| `src/security_tests.rs` | 14 | 1,044 |
| `tests/conversion.rs` | 3 | 46 |
| `tests/reference_fixtures.rs` | 2 | 4 |
| RTF lexer/parser/media unit tests | 17 | 856 |
| Layout unit tests | 9 | 319 |
| PDF renderer/planning unit tests | 5 | 83 |
| PDF audit unit tests | 1 | 18 |
| Model invariant crate tests | 1 | 5 |

The domain files are under `src/security_tests/`, `tests/{conversion,reference}/`, `src/rtf/parser_tests/`, `src/rtf/parser/media/`, `src/layout_tests/`, and `src/pdf_tests/`, beside the renderer-independent audit at `src/pdf/audit_tests.rs` and model-invariant checks in `src/model_invariant_tests.rs`. `tests/test_structure.rs` locks the decomposed inventories so a later move cannot silently drop a test. The remaining crate, CLI, public-facade, black-box security, fuzz-regression, WASM-policy, example, and doctest coverage remains in focused files.

The repository now contains 2,461 `#[test]` functions across library, binary, integration, and example sources. Including the crate doctest, the verified harness inventory is 2,462 tests. The library target contains 2,358 tests, including the 1,044 internal security tests; the remaining focused targets contain 35 CLI, 46 conversion, 5 public-facade, 6 structure-policy, 2 black-box security, 4 reference, 1 fuzz-regression, 2 WASM-policy, and 2 baseline-example tests.

The increase over the earlier 2,402 baseline comes from validation, public inspection, CLI policy, structure guards, media-decoder limits, typed-model invariants, font preparation/caching, and exact PDF planning. Tests removed during typed-model migration covered states that the new representation makes unrepresentable and were replaced by typed exclusivity/invariant assertions.

This is a structural organization only. Test functions and assertions were moved verbatim; relative compile-time fixture paths gained one parent component because the source files are now one directory deeper.
