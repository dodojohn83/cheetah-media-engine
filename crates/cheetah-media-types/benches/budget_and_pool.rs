use cheetah_media_types::{BufferPool, BufferPoolConfig, CopyBudget, CopyReason, SimpleBufferPool};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_copy_budget(c: &mut Criterion) {
    c.bench_function("copy_budget_record", |b| {
        let mut budget = CopyBudget::new(None);
        b.iter(|| {
            budget.record(CopyReason::ParserReassembly, black_box(1024));
        });
    });

    c.bench_function("copy_budget_check", |b| {
        let mut budget = CopyBudget::new(Some(10_000_000));
        for _ in 0..100 {
            budget.record(CopyReason::DemuxToDecoder, 1024);
        }
        b.iter(|| {
            let _ = budget.check();
        });
    });
}

fn bench_buffer_pool(c: &mut Criterion) {
    c.bench_function("buffer_pool_acquire_release", |b| {
        let pool = SimpleBufferPool::new(BufferPoolConfig {
            max_total_bytes: 64 * 1024 * 1024,
            max_count: 1024,
            max_object_size: 1024 * 1024,
            max_wait_ms: 0,
            max_free_count: Some(256),
        });
        b.iter(|| {
            let buf = pool.acquire(black_box(4096)).unwrap();
            drop(buf);
        });
    });

    c.bench_function("buffer_pool_hit_rate", |b| {
        let pool = SimpleBufferPool::new(BufferPoolConfig {
            max_total_bytes: 64 * 1024 * 1024,
            max_count: 1024,
            max_object_size: 1024 * 1024,
            max_wait_ms: 0,
            max_free_count: Some(256),
        });
        // Warmup: seed the free list.
        for _ in 0..32 {
            let buf = pool.acquire(4096).unwrap();
            drop(buf);
        }
        b.iter(|| {
            let buf = pool.acquire(black_box(4096)).unwrap();
            drop(buf);
        });
    });
}

criterion_group!(benches, bench_copy_budget, bench_buffer_pool);
criterion_main!(benches);
