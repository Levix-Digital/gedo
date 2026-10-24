//! Latency of one write-ahead log append of 1 KiB (SC-003: p95 under 1 ms).
//!
//! Criterion measures the mean; the percentiles the requirement names are measured separately
//! over individual appends and printed after the run.

use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_core::ports::WriteAheadLog;
use gedo_store::FileWal;

const PAYLOAD_BYTES: usize = 1024;
const PERCENTILE_SAMPLES: usize = 10_000;

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index]
}

fn report_percentiles() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut wal = FileWal::open(directory.path()).expect("opens");
    let payload = vec![0x42u8; PAYLOAD_BYTES];
    let mut samples: Vec<Duration> = (0..PERCENTILE_SAMPLES)
        .map(|_| {
            let started = Instant::now();
            wal.append(&payload).expect("append");
            started.elapsed()
        })
        .collect();
    samples.sort();
    println!(
        "wal_append 1 KiB over {PERCENTILE_SAMPLES} appends: p50 {:?}, p95 {:?}, p99 {:?}",
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        percentile(&samples, 0.99),
    );
}

fn wal_append(criterion: &mut Criterion) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut wal = FileWal::open(directory.path()).expect("opens");
    let payload = vec![0x42u8; PAYLOAD_BYTES];
    criterion.bench_function("wal_append_1kib", |bencher| {
        bencher.iter(|| wal.append(&payload).expect("append"))
    });
    drop(wal);
    report_percentiles();
}

criterion_group!(benches, wal_append);
criterion_main!(benches);
