# Performance and Deterministic Output Baseline

Status: Active baseline

Captured: 2026-08-09

Converter revision: `b07c86db`

## Purpose

This baseline makes architectural refactors measurable without turning noisy wall-clock timing into a shared-CI pass/fail threshold. It records deterministic PDF properties, the discovered test inventory, the measurement procedure, and the machine context for local performance comparisons.

The deterministic PDF hash, byte size, page count, and indirect-object count are the strongest regression signals. Median duration is diagnostic and must be compared on the same machine under similar load.

## Measurement command

The `measure_baseline` example uses `ConvertOptions::browser_safe_defaults()`, performs warmup conversions, measures repeated conversions with one reused options value, and verifies that every measured conversion returns identical PDF bytes and page counts.

Build and measure the primary fixture from the repository root:

```powershell
cargo run --release --example measure_baseline -- docs/sample.rtf 16 2
```

Arguments are:

```text
<input.rtf> [measured iterations] [warmup iterations]
```

Defaults are 16 measured iterations and 2 warmups. The release build itself is outside the measured interval. The command exits unsuccessfully if conversion fails, PDF bytes change between iterations, page counts change, or the generated PDF cannot be parsed by `lopdf`.

Run the example's calculation tests with:

```powershell
cargo test --example measure_baseline
```

## Captured environment

| Property | Value |
| --- | --- |
| Rust | `rustc 1.96.0 (ac68faa20 2026-05-25)` |
| Cargo | `cargo 1.96.0 (30a34c682 2026-05-25)` |
| OS | Microsoft Windows 11 Home Single Language 10.0.26200 |
| CPU | 11th Gen Intel Core i5-11320H at 3.20 GHz |
| Logical processors | 8 |
| Visible memory | 16,956,841,984 bytes |
| Cargo profile | `release` |
| Conversion options | `ConvertOptions::browser_safe_defaults()` |
| Measured iterations | 16 |
| Warmup iterations | 2 |

Peak memory was not captured because this Windows session did not have a controlled per-process peak-memory harness. Add that measurement later only if it is repeatable and excludes release compilation.

## Deterministic output and timing

| Input | Input bytes | Median ms | PDF bytes | Pages | Indirect objects | PDF SHA-256 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `docs/sample.rtf` | 105,344 | 193.686 | 1,363,655 | 2 | 36 | `a4f80ebeacc220acf3282735f71ecb309e968f159804b4825675b563a58e4a07` |
| `fixtures/tall-table-row-split-passive.rtf` | 2,016 | 33.837 | 126,868 | 4 | 16 | `a539c0181a1190fd4935565a38d70973ee64dc448faf9ce138d326928d685502` |
| `fixtures/wmf-srccopy-cropped-jpeg-dib-passive.rtf` | 3,530 | 5.217 | 96,667 | 1 | 12 | `72513d20114235ef8d9687637011c2edf83e2ff4614e8b7edebc23768c1c2f9f` |
| `fixtures/paragraph-frame-text-direction-passive.rtf` | 1,256 | 13.097 | 106,087 | 6 | 20 | `9349f4a65d58c47aaeaa06484c0aaf3d1065f8a2d829ab1dcce07290af4caa0d` |
| `fixtures/font-code-page-passive.rtf` | 285 | 5.386 | 111,670 | 1 | 10 | `6a848bf3e11485040770d511c98a68e8dea828edb8f3618b965d2d86798afc4a` |

The fixture set covers the general sample, multipage table flow, recovered WMF with JPEG/DIB raster content, framed/rotated text across pages, and legacy font code-page handling.

## Architecture completion verification

The completed 0.2 architecture plan was remeasured on 2026-08-09 with the same primary command and machine. `docs/sample.rtf` produced 1,363,655 bytes, 2 pages, 36 indirect objects, and SHA-256 `a4f80ebeacc220acf3282735f71ecb309e968f159804b4825675b563a58e4a07`, exactly matching the deterministic baseline. The 16-iteration median was 112.598 ms. Timing remains diagnostic; the exact output properties are the release gate.

## Test inventory

The Cargo harness discovered this inventory after adding the two baseline-example tests:

| Target | Tests |
| --- | ---: |
| Library | 1,277 |
| CLI binary | 32 |
| `conversion` | 40 |
| `security` | 1,043 |
| `reference_fixtures` | 4 |
| `fuzz_regression` | 2 |
| `wasm_dependency_policy` | 2 |
| `measure_baseline` example | 2 |
| **Total** | **2,402** |

This inventory was gathered from each Cargo target's `--list` output rather than by counting source annotations. Test decomposition may change target-level counts, but the total must not decrease without an explicit coverage-consolidation record.

## Comparison procedure

For a performance-sensitive or architecture change:

1. Use a clean release build from the candidate revision.
2. Close avoidable CPU-heavy processes and keep the machine on the same power profile.
3. Run the primary command at least twice. Treat the first command as environment stabilization if its medians differ materially.
4. Run the four representative fixture commands with the same iteration and warmup counts.
5. Compare deterministic properties before timing.
6. Investigate any hash, page, object, or byte-size change. Approve it only when the change is intentional and reference/security tests establish equivalence.
7. Compare timing medians only against runs from the same machine. Small changes within normal run-to-run variance are not evidence of improvement or regression.

Representative commands:

```powershell
cargo run --release --example measure_baseline -- fixtures/tall-table-row-split-passive.rtf 16 2
cargo run --release --example measure_baseline -- fixtures/wmf-srccopy-cropped-jpeg-dib-passive.rtf 16 2
cargo run --release --example measure_baseline -- fixtures/paragraph-frame-text-direction-passive.rtf 16 2
cargo run --release --example measure_baseline -- fixtures/font-code-page-passive.rtf 16 2
```

## Updating this baseline

Do not silently replace deterministic values. A baseline update must include:

- the converter commit that caused or approved the change;
- the reason deterministic output changed;
- relevant reference, security, and conversion test evidence;
- refreshed machine/toolchain context if timing is updated;
- old and new performance medians when the change is performance-motivated.

Timing-only variation does not require a committed baseline update. A deterministic hash or structural count change does.
