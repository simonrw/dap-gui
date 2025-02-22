use criterion::{Criterion, criterion_group, criterion_main};
use std::io::{BufReader, Cursor};

use transport::Reader;

pub fn hand_written_parser_benchmark(c: &mut Criterion) {
    let message =
        Cursor::new("Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}\n");

    let mut reader =
        transport::reader::hand_written_reader::HandWrittenReader::new(BufReader::new(message));

    c.bench_function("hand written parser", |b| b.iter(|| reader.poll_message()));
}

criterion_group!(benches, hand_written_parser_benchmark);
criterion_main!(benches);
