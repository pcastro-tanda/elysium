//! CI micro-benchmarks for the parser, the tree walk, and the full lint
//! pipeline, run against three fixtures of increasing size.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use ruby_ast::{walk, Node, Parsed, Visitor};
use ruby_source::SourceFile;

const SMALL: &[u8] = include_bytes!("fixtures/small.rb");
const MEDIUM: &[u8] = include_bytes!("fixtures/medium.rb");
const LARGE: &[u8] = include_bytes!("fixtures/large.rb");

const FIXTURES: [(&str, &[u8]); 3] = [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)];

/// A visitor that counts nodes entered, to keep the walk benchmark from
/// being optimized away.
struct Counter(u32);

impl<'pr> Visitor<'pr> for Counter {
    fn enter(&mut self, _node: &Node<'pr>) {
        self.0 += 1;
    }
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    for (name, bytes) in FIXTURES {
        let source = SourceFile::new(format!("{name}.rb"), bytes.to_vec());
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_function(name, |b| {
            b.iter(|| black_box(Parsed::parse(black_box(&source))));
        });
    }
    group.finish();
}

fn bench_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("walk");
    for (name, bytes) in FIXTURES {
        let source = SourceFile::new(format!("{name}.rb"), bytes.to_vec());
        let parsed = Parsed::parse(&source);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut counter = Counter(0);
                walk(&parsed.root(), &mut counter);
                black_box(counter.0);
            });
        });
    }
    group.finish();
}

fn bench_lint(c: &mut Criterion) {
    let mut group = c.benchmark_group("lint");
    for (name, bytes) in FIXTURES {
        let source = SourceFile::new(format!("{name}.rb"), bytes.to_vec());
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_function(name, |b| {
            b.iter(|| {
                let result = linter::lint_file(&source, &mut linter::NoRules);
                black_box(result.node_count);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse, bench_walk, bench_lint);
criterion_main!(benches);
