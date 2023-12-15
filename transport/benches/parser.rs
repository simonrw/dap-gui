use criterion::{criterion_group, criterion_main, Criterion};
use std::io::{BufReader, Cursor};

use transport::Reader;

pub fn nom_parser_benchmark(c: &mut Criterion) {
    let message =
        Cursor::new("Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\n");

    let mut reader = transport::NomReader::new(BufReader::new(message));

    c.bench_function("nom parser", |b| b.iter(|| reader.poll_message()));
}

pub fn hand_written_parser_benchmark(c: &mut Criterion) {
    let message =
        Cursor::new("Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\n");

    let mut reader = transport::HandWrittenReader::new(BufReader::new(message));

    c.bench_function("hand written parser", |b| b.iter(|| reader.poll_message()));
}

criterion_group!(benches, nom_parser_benchmark, hand_written_parser_benchmark);
criterion_main!(benches);
