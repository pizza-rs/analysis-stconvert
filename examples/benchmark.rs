use std::time::Instant;
use pizza_stconvert::{STConverter, ConvertType};

fn main() {
    let base = "憂鬱的台灣烏龜在陽光下散步，看著遠方的風景，心情非常愉快。";
    let repeat = 1_000_000 / base.len();
    let input: String = base.repeat(repeat);
    let input_bytes = input.len();

    let converter = STConverter::new(ConvertType::T2S);

    // Warmup
    for _ in 0..5 {
        let _ = converter.convert(&input);
    }

    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = converter.convert(&input);
    }
    let elapsed = start.elapsed();
    let total_bytes = input_bytes as f64 * iterations as f64;
    let throughput = total_bytes / elapsed.as_secs_f64() / 1_000_000.0;
    println!("convert():     {:.1} MB/s  ({:.3}ms/iter, input={} bytes)", throughput, elapsed.as_secs_f64() * 1000.0 / iterations as f64, input_bytes);

    // Buffer reuse
    let mut buf = String::with_capacity(input_bytes);
    let start = Instant::now();
    for _ in 0..iterations {
        buf.clear();
        converter.convert_to(&input, &mut buf);
    }
    let elapsed = start.elapsed();
    let throughput2 = total_bytes / elapsed.as_secs_f64() / 1_000_000.0;
    println!("convert_to():  {:.1} MB/s  ({:.3}ms/iter)", throughput2, elapsed.as_secs_f64() * 1000.0 / iterations as f64);

    // ASCII passthrough
    let ascii_input = "hello world this is pure ascii text ok ".repeat(repeat);
    let ascii_bytes = ascii_input.len();
    let start = Instant::now();
    for _ in 0..iterations {
        buf.clear();
        converter.convert_to(&ascii_input, &mut buf);
    }
    let elapsed = start.elapsed();
    let tp3 = ascii_bytes as f64 * iterations as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    println!("ASCII pass:    {:.1} MB/s  (input={} bytes)", tp3, ascii_bytes);
}
