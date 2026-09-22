//! Benchmark and throughput measurement harness for blnk core operations.

use blnk::protocol::swsp::{Frame, FrameFlags};
use std::time::Instant;

fn bench_swsp_framing() {
    let iterations = 100_000;
    let payload = vec![0x42u8; 1024]; // 1 KB
    let frame = Frame::new(1, FrameFlags::DAT, payload);

    let start = Instant::now();
    let mut total_bytes = 0usize;
    for _ in 0..iterations {
        let encoded = frame.encode().expect("encode should succeed");
        total_bytes += encoded.len();
        let (decoded, _) = Frame::decode(&encoded).expect("decode should succeed");
        assert_eq!(decoded.stream_id, 1);
    }
    let elapsed = start.elapsed();
    let throughput_mb = (total_bytes as f64) / (1024.0 * 1024.0) / elapsed.as_secs_f64();
    println!(
        "[bench] SWSP Framing: {} iterations in {:.2?} ({:.2} MB/s)",
        iterations, elapsed, throughput_mb
    );
}

fn bench_crypto_primitives() {
    let iterations = 10_000;
    let nonce = vec![0x11u8; 32];
    let start = Instant::now();
    for _ in 0..iterations {
        let commit = blnk::protocol::pairing::commitment_for(&nonce);
        assert!(!commit.is_empty());
    }
    let elapsed = start.elapsed();
    println!(
        "[bench] Commitment Derivation: {} iterations in {:.2?}",
        iterations, elapsed
    );
}

fn main() {
    println!("=== blnk Performance Benchmarks ===");
    bench_swsp_framing();
    bench_crypto_primitives();
}
