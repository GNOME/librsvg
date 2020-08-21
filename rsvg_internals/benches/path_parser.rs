#[macro_use]
extern crate criterion;
use criterion::{black_box, Criterion};

use rsvg_internals::path_builder::PathBuilder;
use rsvg_internals::path_parser::{parse_path_into_builder, Lexer};

static INPUT: &'static str = "M10 20 C 30,40 50 60-70,80,90 100,110 120,130,140";

static BYTES: &'static [u8; 49] = b"M10 20 C 30,40 50 60-70,80,90 100,110 120,130,140";

static SLICE_EDGES: [(usize, usize); 14] = [
    (1, 3),
    (4, 6),
    (9, 11),
    (12, 14),
    (15, 17),
    (18, 20),
    (20, 23),
    (24, 26),
    (27, 29),
    (30, 33),
    (34, 37),
    (38, 41),
    (42, 45),
    (46, 49),
];

fn lex_path(input: &str) {
    let lexer = Lexer::new(black_box(INPUT));

    for (_pos, _token) in lexer {
        // no-op
    }
}

fn path_parser(c: &mut Criterion) {
    c.bench_function("parse path into builder", |b| {
        let input = black_box(INPUT);
        let mut builder = PathBuilder::default();

        b.iter(|| {
            let _ = parse_path_into_builder(&input, &mut builder);
        });
    });

    c.bench_function("lex str", |b| {
        let input = black_box(INPUT);

        b.iter(|| {
            lex_path(input);
        });
    });

    // look at how much time *just* the parse::<i32> part of the lexer should be taking...
    c.bench_function("std i32 parse (bytes)", |b| {
        let input = black_box(BYTES);
        let slice_boundaries = black_box(SLICE_EDGES);

        b.iter(|| {
            for (a, b) in slice_boundaries.iter() {
                let a: usize = *a;
                let b: usize = *b;
                unsafe {
                    let _ = std::str::from_utf8_unchecked(&input[a..b])
                        .parse::<i32>()
                        .unwrap();
                }
            }
        });
    });
}

criterion_group!(benches, path_parser);
criterion_main!(benches);
