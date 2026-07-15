# Cheetah Media Engine Web v1 Benchmark Report

Generated: 2026-07-15T14:42:51.811Z
Commit: unknown
Source: `cargo bench -p cheetah-media-types --features std`

| Benchmark | Mean | 95% CI | Std Dev | Samples |
|-------------|------|--------|---------|----------|
| buffer_pool_acquire_release | 98.272 ns | 97.847 ns–98.753 ns | 2.329 ns | 100 |
| buffer_pool_hit_rate | 97.843 ns | 97.476 ns–98.252 ns | 1.988 ns | 100 |
| copy_budget_check | 2.463 ns | 2.422 ns–2.503 ns | 0.209 ns | 100 |
| copy_budget_record | 2.549 ns | 2.536 ns–2.565 ns | 0.076 ns | 100 |

## Notes

- Times are per-iteration and are produced by Criterion.rs on the local VM.
- These are baseline measurements; target hardware will differ.
- Raw sample data is in `target/criterion/<benchmark>/new/sample.json`.
