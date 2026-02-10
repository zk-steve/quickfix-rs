use std::{
    hint::black_box,
    sync::Mutex,
    time::{Duration, Instant},
};

use quickfix::{FieldMap, Group, Message, QuickFixError};

static GLOBAL_LOCK: Mutex<u8> = Mutex::new(0);

const WARMUP_ITERS: usize = 2_000;
const BENCH_ITERS: usize = 20_000;

#[derive(Debug)]
struct BenchResult {
    iterations: usize,
    to_fix_string: Duration,
    to_fix_str: Duration,
}

fn build_sample_message() -> Result<Message, QuickFixError> {
    let mut msg = Message::new();

    // Header values are intentionally set to resemble a realistic app message.
    msg.with_header_mut(|h| -> Result<(), QuickFixError> {
        h.set_field(8, "FIX.4.4")?;
        h.set_field(49, "SENDER")?;
        h.set_field(56, "TARGET")?;
        Ok(())
    })?;

    msg.set_field(35, "D")?;
    msg.set_field(11, "ORDER-12345")?;
    msg.set_field(55, "AAPL")?;
    msg.set_field(54, 1)?;
    msg.set_field(38, 100)?;
    msg.set_field(44, 189.42)?;

    // Add a small repeating group to include non-trivial serialization work.
    let mut party_1 = Group::try_new(453, 448)?;
    party_1.set_field(448, "PARTY-1")?;
    party_1.set_field(447, "D")?;
    party_1.set_field(452, 1)?;
    msg.add_group(&party_1)?;

    let mut party_2 = party_1.clone();
    party_2.set_field(448, "PARTY-2")?;
    party_2.set_field(452, 3)?;
    msg.add_group(&party_2)?;

    Ok(msg)
}

fn run_bench(iterations: usize) -> Result<BenchResult, QuickFixError> {
    let _lock = GLOBAL_LOCK.lock().expect("GLOBAL_LOCK poisoned");

    let mut msg_for_str = build_sample_message()?;
    let msg_for_string = msg_for_str.clone();

    for _ in 0..WARMUP_ITERS {
        let text = msg_for_string.to_fix_string()?;
        black_box(text.len());
    }

    for _ in 0..WARMUP_ITERS {
        let text = msg_for_str.to_fix_str()?;
        black_box(text.len());
    }

    let t0 = Instant::now();
    for _ in 0..iterations {
        let text = msg_for_string.to_fix_string()?;
        black_box(text.len());
    }
    let to_fix_string = t0.elapsed();

    let t1 = Instant::now();
    for _ in 0..iterations {
        let text = msg_for_str.to_fix_str()?;
        black_box(text.len());
    }
    let to_fix_str = t1.elapsed();

    Ok(BenchResult {
        iterations,
        to_fix_string,
        to_fix_str,
    })
}

#[test]
#[ignore = "Manual benchmark; run with -- --ignored --nocapture"]
fn bench_to_fix_string_vs_to_fix_str() -> Result<(), QuickFixError> {
    let result = run_bench(BENCH_ITERS)?;

    let string_ns_per_op = result.to_fix_string.as_nanos() as f64 / result.iterations as f64;
    let str_ns_per_op = result.to_fix_str.as_nanos() as f64 / result.iterations as f64;
    let delta_pct = ((result.to_fix_string.as_nanos() as f64
        - result.to_fix_str.as_nanos() as f64)
        / result.to_fix_string.as_nanos() as f64)
        * 100.0;

    println!("=== FIX serialization benchmark ===");
    println!("iterations: {}", result.iterations);
    println!(
        "1) to_fix_string (allocating): {:?} ({:.0} ns/op)",
        result.to_fix_string, string_ns_per_op
    );
    println!(
        "2) to_fix_str (borrowed):      {:?} ({:.0} ns/op)",
        result.to_fix_str, str_ns_per_op
    );
    println!("delta vs #1: {:.2}%", delta_pct);

    Ok(())
}
