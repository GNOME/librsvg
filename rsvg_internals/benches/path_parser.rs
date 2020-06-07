#[macro_use]
extern crate criterion;
use criterion::{black_box, Criterion};

use rsvg_internals::path_builder::PathBuilder;
use rsvg_internals::path_parser::parse_path_into_builder;

static INPUT: &'static str = "M10 20 C 30,40 50 60-70,80,90 100,110 120,130,140";

fn path_parser(c: &mut Criterion) {
    c.bench_function("parse path into builder", |b| {
        let input = black_box(INPUT);
        let mut builder = PathBuilder::new();

        b.iter(|| {
            let _ = parse_path_into_builder(&input, &mut builder);
        });
    });
}

criterion_group!(benches, path_parser);
criterion_main!(benches);
