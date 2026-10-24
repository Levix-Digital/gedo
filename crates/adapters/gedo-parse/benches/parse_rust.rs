//! Parse, normalization and hashing of a 2,000-line Rust file (SC-001: p95 under 10 ms).
//!
//! The input repeats the conformance fixture `src/lib.rs` inside numbered modules until it has
//! 2,000 lines. Criterion measures the mean; the percentiles the requirement names are measured
//! separately over individual runs and printed after the run.

use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_core::ports::Parser;
use gedo_parse::TreeSitterParser;

const TARGET_LINES: usize = 2_000;
const PERCENTILE_SAMPLES: usize = 500;
const FIXTURE: &str = include_str!("../tests/fixtures/rust/src/lib.rs");

fn input() -> Vec<u8> {
    let body: String = FIXTURE
        .lines()
        .filter(|line| !line.starts_with("//!") && !line.starts_with("#!["))
        .map(|line| format!("    {line}\n"))
        .collect();
    let mut source = String::new();
    let mut copy = 0;
    while source.lines().count() < TARGET_LINES {
        source.push_str(&format!("mod copy_{copy} {{\n{body}}}\n"));
        copy += 1;
    }
    let mut lines: Vec<&str> = source.lines().take(TARGET_LINES).collect();
    // Keep the last module closed when the cut falls inside it.
    lines.push("}");
    lines.join("\n").into_bytes()
}

fn run(parser: &TreeSitterParser, source: &[u8]) {
    let parsed = parser.parse("src/bench.rs", source).expect("parses");
    gedo_ast::normalize(&parsed, source).expect("normalizes");
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index]
}

fn report_percentiles(parser: &TreeSitterParser, source: &[u8]) {
    let mut samples: Vec<Duration> = (0..PERCENTILE_SAMPLES)
        .map(|_| {
            let started = Instant::now();
            run(parser, source);
            started.elapsed()
        })
        .collect();
    samples.sort();
    println!(
        "parse_rust {TARGET_LINES} lines over {PERCENTILE_SAMPLES} runs: p50 {:?}, p95 {:?}, p99 {:?}",
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        percentile(&samples, 0.99),
    );
}

fn parse_rust(criterion: &mut Criterion) {
    let parser = TreeSitterParser::new();
    let source = input();
    criterion.bench_function("parse_rust_2000_lines", |bencher| {
        bencher.iter(|| run(&parser, &source))
    });
    report_percentiles(&parser, &source);
}

criterion_group!(benches, parse_rust);
criterion_main!(benches);
