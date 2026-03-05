use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quickfix::{FieldMap, Message};
use quickfix_ffi::{FixBenchmark_noop, FixBenchmark_strEqual};

const FIELD_TAG: i32 = 11;
const FIELD_VALUE: &str = "ORDER-12345";

fn build_message() -> Message {
    let mut msg = Message::new();
    msg.set_field(FIELD_TAG, FIELD_VALUE)
        .expect("failed to set benchmark field");
    msg
}

fn bench_field_compare(c: &mut Criterion) {
    let msg = build_message();
    let preloaded_field = msg
        .get_field_str(FIELD_TAG)
        .expect("benchmark field must exist")
        .to_owned();
    let preloaded_field_bytes = preloaded_field.as_bytes();
    let expected_bytes = FIELD_VALUE.as_bytes();

    let mut overhead = c.benchmark_group("field_compare_overhead");

    overhead.bench_function("rust_native_preloaded_compare", |b| {
        b.iter(|| {
            let lhs = black_box(preloaded_field.as_str());
            let rhs = black_box(FIELD_VALUE);
            let is_equal = lhs == rhs;
            black_box(is_equal);
        });
    });

    overhead.bench_function("rust_to_c_noop_call", |b| {
        b.iter(|| {
            let is_ok = unsafe { FixBenchmark_noop() == 1 };
            black_box(is_ok);
        });
    });

    overhead.bench_function("rust_to_c_raw_compare", |b| {
        b.iter(|| {
            let lhs_ptr = black_box(preloaded_field_bytes.as_ptr().cast());
            let lhs_len = black_box(preloaded_field_bytes.len() as u64);
            let rhs_ptr = black_box(expected_bytes.as_ptr().cast());
            let rhs_len = black_box(expected_bytes.len() as u64);
            let is_equal =
                unsafe { FixBenchmark_strEqual(lhs_ptr, lhs_len, rhs_ptr, rhs_len) == 1 };
            black_box(is_equal);
        });
    });

    overhead.finish();

    let mut group = c.benchmark_group("field_compare_end_to_end");

    group.bench_function("get_field_string_then_compare", |b| {
        b.iter(|| {
            let is_equal = msg.get_field(FIELD_TAG).as_deref() == Some(FIELD_VALUE);
            black_box(is_equal);
        });
    });

    group.bench_function("get_field_str_then_compare", |b| {
        b.iter(|| {
            let is_equal = msg
                .get_field_str(FIELD_TAG)
                .is_some_and(|value| value == FIELD_VALUE);
            black_box(is_equal);
        });
    });

    group.bench_function("is_field_equal_cpp_compare", |b| {
        b.iter(|| {
            let is_equal = msg.is_field_equal(FIELD_TAG, FIELD_VALUE);
            black_box(is_equal);
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench_field_compare
}
criterion_main!(benches);
