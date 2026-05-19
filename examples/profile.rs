use std::time::Instant;
use std::hint::black_box;
use pizza_stconvert::{STConverter, ConvertType};

fn main() {
    let base = "憂鬱的台灣烏龜在陽光下散步看著遠方的風景心情非常愉快";
    let repeat = 1_000_000 / base.len();
    let input: String = base.repeat(repeat);
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    
    let converter = STConverter::new(ConvertType::T2S);

    // Test 1: Just iterate chars (baseline)
    let start = Instant::now();
    for _ in 0..100 {
        let mut sum = 0u32;
        for &c in &chars {
            sum = sum.wrapping_add(c as u32);
        }
        black_box(sum);
    }
    let t1 = start.elapsed();
    println!("Char iteration only: {:.1}ms/iter ({} chars)", t1.as_secs_f64()*1000.0/100.0, n);

    // Test 2: Full convert
    let start = Instant::now();
    for _ in 0..100 {
        black_box(converter.convert(&input));
    }
    let t2 = start.elapsed();
    println!("Full convert:        {:.1}ms/iter", t2.as_secs_f64()*1000.0/100.0);
    
    // Test 3: convert_to (buffer reuse)
    let mut buf = String::with_capacity(input.len());
    let start = Instant::now();
    for _ in 0..100 {
        buf.clear();
        converter.convert_to(&input, &mut buf);
    }
    let t3 = start.elapsed();
    println!("convert_to (reuse):  {:.1}ms/iter", t3.as_secs_f64()*1000.0/100.0);
    
    println!("\nInput: {} bytes, {} chars", input.len(), n);
    let throughput = input.len() as f64 * 100.0 / t3.as_secs_f64() / 1_000_000.0;
    println!("Throughput: {:.1} MB/s", throughput);
}
