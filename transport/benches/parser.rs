use criterion::{criterion_group, criterion_main, Criterion};
use std::io::{BufReader, Cursor};

use transport::Reader;

pub fn parser_benchmark(c: &mut Criterion) {
    let message =
        Cursor::new("Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\n");

    let mut reader = Reader::new(BufReader::new(message));

    c.bench_function("poll message", |b| b.iter(|| reader.poll_message()));
}

criterion_group!(benches, parser_benchmark);
criterion_main!(benches);
