use clap::Parser;
use rayon::prelude::*;

/// Test and benchmark program for alternative ilog10 implementations.
#[derive(Parser, Debug)]
struct Args {
    /// Run an exhaustive u32 test
    #[arg(short, long)]
    test: bool,
}

fn main() {
    let args = Args::parse();
    if args.test {
        test_ilog();
    } else {
        benchmark_ilog();
    }
}

fn test_ilog() {
    // h/t @steffahn for suggesting using rayon to parallelize. goes brrr.
    let start = std::time::Instant::now();
    (1..=u32::MAX)
        .into_par_iter()
        .for_each(|x| assert_eq!(ilog10(x), x.ilog10()));
    let elapsed = start.elapsed();
    println!(
        "passed exhaustive u32 test in {:.2} seconds",
        elapsed.as_secs_f64()
    );
}

/// Reference version copied from Rust stdlib.
#[inline]
const fn less_than_5(val: u32) -> u32 {
    // Similar to u8, when adding one of these constants to val,
    // we get two possible bit patterns above the low 17 bits,
    // depending on whether val is below or above the threshold.
    const C1: u32 = 0b011_00000000000000000 - 10; // 393206
    const C2: u32 = 0b100_00000000000000000 - 100; // 524188
    const C3: u32 = 0b111_00000000000000000 - 1000; // 916504
    const C4: u32 = 0b100_00000000000000000 - 10000; // 514288

    // Value of top bits:
    //                +c1  +c2  1&2  +c3  +c4  3&4   ^
    //         0..=9  010  011  010  110  011  010  000 = 0
    //       10..=99  011  011  011  110  011  010  001 = 1
    //     100..=999  011  100  000  110  011  010  010 = 2
    //   1000..=9999  011  100  000  111  011  011  011 = 3
    // 10000..=99999  011  100  000  111  100  100  100 = 4
    (((val + C1) & (val + C2)) ^ ((val + C3) & (val + C4))) >> 17
}

pub const fn ilog10_u32(mut val: u32) -> u32 {
    let mut log = 0;
    if val >= 100_000 {
        val /= 100_000;
        log += 5;
    }
    log + less_than_5(val)
}

/// dga version with speedup from @sahnehaeubchen

const TEN_THRESHOLDS: [u32; 10] = [
9,
99,
999,
9999,
99999,
999999,
9999999,
99999999,
999_999_999,
u32::MAX,
];

// dave's popcount version that only works really well on AMD EPYC. :)
#[inline]
const fn ilogpopc(val_lz: u32) -> u32 {
    const LZ_GUESSMASK: u32 = 0b01001001000100100100010010010000;
    let guess = (LZ_GUESSMASK << val_lz).count_ones();
    if guess > LZ_GUESSMASK.count_ones() {
        // SAFETY: shifting never increases the count of ones
        unsafe { std::hint::unreachable_unchecked() }
    }
    guess
}

const fn ilog10(val: u32) -> u32 {
    let guess = ilogpopc(val.leading_zeros());
    let ttg = TEN_THRESHOLDS[guess as usize];
    guess + (val > ttg) as u32
}


// version from quaternic on the rust forum
pub const fn ilog10_mul_or(x: u32) -> u32 {
    // set least significant 3 bits so numbers 0 to 6 all get the same treatment 7
    // changes nothing if x >= 7
    let log2 = ((x | 7) >> 1).ilog2();
    debug_assert!(log2 < 31);
    // guess close enough for all u32
    let guess = log2.wrapping_mul(5) >> 4;
    debug_assert!(guess < 10);
    if guess >= 10 {
        unsafe { std::hint::unreachable_unchecked() }
    }
    let ttg = TEN_THRESHOLDS[guess as usize];
    guess + (x > ttg) as u32
}


// hacker's delight version borrowing optimizations 
// from the rust forum discussion.
pub const fn ilog10_mul(x: u32) -> u32 {
    let guess = x.ilog2().wrapping_mul(9) >> 5;
    debug_assert!(guess < 10);
    if guess >= 10 {
        unsafe { std::hint::unreachable_unchecked() }
    }
    let ttg = TEN_THRESHOLDS[guess as usize];
    guess + (x > ttg) as u32
}

// hacker's delight borrowing optimization idea from scottmcm@rustforum
// to ensure the table access is unchecked. Seems to save a bounds check
// standalone but that may get optimized away when used with ilog10_checked.
pub fn ilog10_mul_alt(x: u32) -> u32 {
    let guess = (x.ilog2() * 9) >> 5;
    let ttg = unsafe { *TEN_THRESHOLDS.get_unchecked(guess as usize) };
    guess + (x > ttg) as u32
}

fn runloop<F>(f: &F) -> u128
where
    F: Fn(u32) -> u32,
{
    const LOOPS: usize = 1;
    const UPTO: u32 = u32::MAX;
    let start = std::time::Instant::now();
    for _ in 0..LOOPS {
        for i in 1..=UPTO {
            std::hint::black_box(f(i));
        }
    }
    start.elapsed().as_micros()
}

fn benchmark_ilog() {
    let elapsed_real = runloop(&ilog10_u32);
    let elapsed_popc = runloop(&ilog10);
    let elapsed_mul = runloop(&ilog10_mul);
    println!("|Platform | popcount | mul | stdlib |");
    println!("|---------|----------|-----|--------|");
    println!("|  |  {elapsed_popc} | {elapsed_mul} | {elapsed_real} |");
    println!("");
}
