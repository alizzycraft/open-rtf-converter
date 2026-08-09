# Test structure

The large test targets keep their established Cargo target names while test functions live in domain files. Shared fixtures and helper functions remain in each target's root module so the split does not duplicate support code or alter assertions.

| Target or subsystem | Domain files | Preserved tests |
| --- | ---: | ---: |
| `tests/security.rs` | 14 | 1,043 |
| `tests/conversion.rs` | 3 | 44 |
| `tests/reference_fixtures.rs` | 2 | 4 |
| RTF parser unit tests | 11 | 845 |
| RTF media decoder unit tests | 5 | 5 |
| Layout unit tests | 9 | 319 |
| PDF renderer unit tests | 5 | 78 |
| PDF audit unit tests | 1 | 18 |

The domain files are under `tests/{security,conversion,reference}/`, `src/rtf/parser_tests/`, `src/rtf/parser/media/`, `src/layout_tests/`, `src/pdf_tests/`, and beside the renderer-independent audit at `src/pdf/audit_tests.rs`. `tests/test_structure.rs` locks these inventories so a later move cannot silently drop a test. The remaining crate, CLI, public-facade, fuzz-regression, WASM-policy, example, and doctest coverage remains in its existing focused files.

The repository now contains 2,430 `#[test]` functions. Including the crate doctest and the two baseline-example tests, the verified harness inventory is 2,433 tests; the increase over the earlier 2,402 baseline comes from the validation, public-inspection, CLI-policy, structure-guard, and explicit media-decoder limit tests added by plan items.

This is a structural organization only. Test functions and assertions were moved verbatim; relative compile-time fixture paths gained one parent component because the source files are now one directory deeper.
