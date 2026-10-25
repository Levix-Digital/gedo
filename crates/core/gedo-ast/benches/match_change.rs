//! Matching the entities of a 2,000-line file against its previous version (SC-004: p95 under
//! 5 ms).
//!
//! The input repeats the conformance fixture `src/lib.rs` inside numbered modules until it has
//! 2,000 lines, like the parse benchmark of feature 002. The new version renames one function and
//! adds a statement to the body of every tenth one, so all tiers run. Parsing is outside the
//! measurement. Criterion measures the mean; the percentiles are printed after the run.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use gedo_ast::{ChangeSet, EntityTokens, NewFile, OldFile, match_change};
use gedo_core::ids::GitOid;
use gedo_core::ports::Parser;
use gedo_core::syntax::SyntaxMap;
use gedo_parse::TreeSitterParser;

const TARGET_LINES: usize = 2_000;
const PERCENTILE_SAMPLES: usize = 500;
const PATH: &str = "src/bench.rs";
const FIXTURE: &str = include_str!("../../../adapters/gedo-parse/tests/fixtures/rust/src/lib.rs");

fn old_source() -> String {
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
    lines.join("\n")
}

/// The old source with one function renamed and a statement added to every tenth body.
fn new_source(old: &str) -> String {
    let mut functions = 0;
    let mut source = String::new();
    for line in old.lines() {
        source.push_str(line);
        source.push('\n');
        let opens_body = line.trim_end().ends_with('{');
        if opens_body && (line.trim_start().starts_with("fn ") || line.contains(" fn ")) {
            functions += 1;
            if functions % 10 == 0 {
                source.push_str("        let gedo_bench_probe = 1;\n");
            }
        }
    }
    let first = source.find("fn ").expect("the fixture has functions") + 3;
    source.insert_str(
        first + source[first..].find('(').expect("a parameter list"),
        "_renamed",
    );
    source
}

fn syntax(source: &str) -> SyntaxMap {
    let parsed = TreeSitterParser::new()
        .parse(PATH, source.as_bytes())
        .expect("parses");
    gedo_ast::normalize(&parsed, source.as_bytes()).expect("normalizes")
}

fn blob(source: &str) -> GitOid {
    GitOid::from_bytes(&blake3::hash(source.as_bytes()).as_bytes()[..20]).expect("20 bytes")
}

/// The change set of the edit: the old file anchored by a first match, the new one parsed.
fn change_set() -> ChangeSet {
    let old = old_source();
    let new = new_source(&old);
    let old_syntax = syntax(&old);
    let first = ChangeSet {
        old: Vec::new(),
        new: vec![NewFile {
            blob: blob(&old),
            syntax: old_syntax.clone(),
        }],
    };
    let map = match_change(&first, &BTreeSet::new())
        .maps
        .pop()
        .expect("one map");
    let tokens = old_syntax
        .entities
        .iter()
        .map(|entity| Some(EntityTokens::of(entity)))
        .collect();
    ChangeSet {
        old: vec![OldFile { map, tokens }],
        new: vec![NewFile {
            blob: blob(&new),
            syntax: syntax(&new),
        }],
    }
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index]
}

fn report_percentiles(change: &ChangeSet, known: &BTreeSet<gedo_core::identity::AstNodeId>) {
    let mut samples: Vec<Duration> = (0..PERCENTILE_SAMPLES)
        .map(|_| {
            let started = Instant::now();
            match_change(change, known);
            started.elapsed()
        })
        .collect();
    samples.sort();
    println!(
        "match_change {TARGET_LINES} lines over {PERCENTILE_SAMPLES} runs: p50 {:?}, p95 {:?}, p99 {:?}",
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        percentile(&samples, 0.99),
    );
}

fn bench(criterion: &mut Criterion) {
    let change = change_set();
    let known = BTreeSet::new();
    criterion.bench_function("match_change_2000_lines", |bencher| {
        bencher.iter(|| match_change(&change, &known))
    });
    report_percentiles(&change, &known);
}

criterion_group!(benches, bench);
criterion_main!(benches);
