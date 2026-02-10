use std::{
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use quickfix::{
    send_to_target, send_to_target_by_ref_mut, Acceptor, Application, ConnectionHandler, FieldMap,
    FixSocketServerKind, Initiator, LogFactory, MemoryMessageStoreFactory, NullLogger,
    QuickFixError,
};
use utils::{
    build_news, build_settings, find_available_port, NullFixApplication, ServerType, MSG_HEADLINE,
};

mod utils;

static GLOBAL_LOCK: Mutex<u8> = Mutex::new(0);

const WARMUP_ITERS: usize = 200;
const BENCH_ITERS: usize = 2_000;

#[derive(Debug)]
struct BenchResult {
    iterations: usize,
    string_then_send: Duration,
    str_then_send_by_ref_mut: Duration,
}

fn run_bench(
    iterations: usize,
    server_kind: FixSocketServerKind,
) -> Result<BenchResult, QuickFixError> {
    let _lock = GLOBAL_LOCK.lock().expect("GLOBAL_LOCK poisoned");

    let communication_port = find_available_port();
    let settings_sender = build_settings(ServerType::Sender, communication_port)?;
    let settings_receiver = build_settings(ServerType::Receiver, communication_port)?;

    let log_factory = LogFactory::try_new(&NullLogger)?;

    let app_sender = Application::try_new(&NullFixApplication)?;
    let app_receiver = Application::try_new(&NullFixApplication)?;

    let message_store_factory_sender = MemoryMessageStoreFactory::new();
    let message_store_factory_receiver = MemoryMessageStoreFactory::new();

    let mut socket_sender = Initiator::try_new(
        &settings_sender,
        &app_sender,
        &message_store_factory_sender,
        &log_factory,
        server_kind,
    )?;
    let mut socket_receiver = Acceptor::try_new(
        &settings_receiver,
        &app_receiver,
        &message_store_factory_receiver,
        &log_factory,
        server_kind,
    )?;

    socket_receiver.start()?;
    socket_sender.start()?;

    while !socket_sender.is_logged_on()? || !socket_receiver.is_logged_on()? {
        thread::sleep(Duration::from_millis(10));
    }

    for _ in 0..WARMUP_ITERS {
        let msg = build_news("warmup", &[])?;
        let _headline = msg.get_field(MSG_HEADLINE);
        send_to_target(msg, &ServerType::Sender.session_id())?;
    }

    for _ in 0..WARMUP_ITERS {
        let mut msg = build_news("warmup", &[])?;
        let _headline = msg.get_field_str(MSG_HEADLINE);
        send_to_target_by_ref_mut(&mut msg, &ServerType::Sender.session_id())?;
        drop(msg);
    }

    thread::sleep(Duration::from_millis(100));

    let t0 = Instant::now();
    for _ in 0..iterations {
        let msg = build_news("string_send", &[])?;
        let _headline = msg.get_field(MSG_HEADLINE);
        send_to_target(msg, &ServerType::Sender.session_id())?;
    }
    let string_then_send = t0.elapsed();

    thread::sleep(Duration::from_millis(100));

    let t1 = Instant::now();
    for _ in 0..iterations {
        let mut msg = build_news("str_send_ref_mut", &[])?;
        let _headline = msg.get_field_str(MSG_HEADLINE);
        send_to_target_by_ref_mut(&mut msg, &ServerType::Sender.session_id())?;
        drop(msg);
    }
    let str_then_send_by_ref_mut = t1.elapsed();

    thread::sleep(Duration::from_millis(100));

    socket_receiver.stop()?;
    socket_sender.stop()?;

    Ok(BenchResult {
        iterations,
        string_then_send,
        str_then_send_by_ref_mut,
    })
}

#[test]
#[ignore = "Manual benchmark; run with -- --ignored --nocapture"]
fn bench_get_field_then_send_paths() -> Result<(), QuickFixError> {
    let result = run_bench(BENCH_ITERS, FixSocketServerKind::SingleThreaded)?;

    let string_per_op_ns = result.string_then_send.as_nanos() as f64 / result.iterations as f64;
    let str_per_op_ns =
        result.str_then_send_by_ref_mut.as_nanos() as f64 / result.iterations as f64;
    let delta_pct = ((result.string_then_send.as_nanos() as f64
        - result.str_then_send_by_ref_mut.as_nanos() as f64)
        / result.string_then_send.as_nanos() as f64)
        * 100.0;

    println!("=== send_to_target benchmark ===");
    println!("iterations: {}", result.iterations);
    println!(
        "1) get_field(String) + send_to_target(consuming): {:?} ({:.0} ns/op)",
        result.string_then_send, string_per_op_ns
    );
    println!(
        "2) get_field_str(&str) + send_to_target_by_ref_mut + drop: {:?} ({:.0} ns/op)",
        result.str_then_send_by_ref_mut, str_per_op_ns
    );
    println!("delta vs #1: {:.2}%", delta_pct);

    Ok(())
}
