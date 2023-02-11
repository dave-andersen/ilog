use clap::Parser;
use rand::prelude::*;
use rayon::prelude::*;

/// Test and benchmark program for alternative ilog10 implementations.
#[derive(Parser, Debug)]
struct Args {
    /// Run an exhaustive u32 test
    #[arg(short, long)]
    test: bool,

    #[arg(short, long)]
    testu64: bool,
}

fn main() {
    let args = Args::parse();
    if args.testu64 {
        test_ilog64();
        return;
    }
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
        .for_each(|x| assert_eq!(ilog10_mul(x), x.ilog10()));
    let elapsed = start.elapsed();
    println!(
        "passed exhaustive u32 test in {:.2} seconds",
        elapsed.as_secs_f64()
    );
}

// the warren mapping follows a slightly unintuitive invariant:
// The warren map value must be correctable to the real log10 value
// with the addition of at most 1.
fn test_warren_64bit() {
    let mut test_values: Vec<u64> = (0..62).map(|x| 1u64 << x).collect();
    for i in 2..64 {
        test_values.push(((1u128 << i) - 1) as u64);
    }
    test_values.push(u64::MAX);

    for val in test_values {
        let log2val = val.ilog2();
        let real_log10val = val.ilog10();
        // This is unfortunate. The cheap warren map doesn't work.
        // We have to mul by 19, which turns into
        // x << 4 + x << 1 + x
        // which is a little more expensive.
        // on x64 it's .. two lea's. *grin* not bad at all.
        let warren_map = log2val.wrapping_mul(19) >> 6;
        assert!(warren_map == real_log10val || warren_map == real_log10val - 1);
    }
}

fn test_ilog64() {
    println!("Testing warren mapping function");
    test_warren_64bit();
    println!("Testing log of u32s to sanity check");
    let start = std::time::Instant::now();
    (1..=u32::MAX)
        .into_par_iter()
        .map(|x| x as u64)
        .for_each(|x| assert_eq!(ilog10_u64_mul(x), x.ilog10()));
    let elapsed = start.elapsed();
    println!(
        "passed exhaustive u32 test in {:.2} seconds",
        elapsed.as_secs_f64()
    );
    println!("Testing boundary values");
    assert_eq!(ilog10_u64_mul(1u64 << 62), (1u64 << 62).ilog10());
    assert_eq!(ilog10_u64_mul(u64::MAX), u64::MAX.ilog10());
    // Now test the 64 bit version using random 64 bit values
    println!("Testing random u64s");
    let start = std::time::Instant::now();
    (1..128).into_par_iter().for_each(|_| {
        let mut rng = rand::thread_rng();
        for _ in 0..10000000 {
            let x = rng.gen::<u64>();
            assert_eq!(ilog10_u64_mul(x), x.ilog10());
        }
    });
    let elapsed = start.elapsed();
    println!(
        "passed random u64 test in {:.2} seconds",
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

// The following functions mostly combine two parts:
// (1) A guess for ilog10 based on ilog2 or leading zeros;
// (2) A correction based on a lookup table listing powers
// of ten. The major differences are in the guess function,
// as most optimizations to the lookup table are common.
// Guess functions:
// dave shift/popcount - 2 instructions but popcount is slow on many arch
// warren x*9 >> 5 version - 2 instructions on x64 (lea + shr), all fast.
// The dave shift one can use the results of lzcnt directly, whereas
// the warren one needs to be 31 - lzcnt (one more xor). Mostly unimportant
// difference as the popcnt cost dominates everywhere but AMD.

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
    if val == 0 {
        // SAFETY: This is ensured by our caller
        unsafe {
            std::hint::unreachable_unchecked();
        }
    }
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

pub fn log10_table_table(x: u32) -> u32 {
    const guess_table: [u8; 33] = [
        0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8,
        9, 9, 9,
    ];
    const thresholds: [u32; 10] = [
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

    let log2 = x.ilog2();
    let guess = guess_table[log2 as usize] as u32;
    guess + (x > thresholds[guess as usize]) as u32
}

// hacker's delight borrowing optimization idea from scottmcm@rustforum
// to ensure the table access is unchecked. Seems to save a bounds check
// standalone but that may get optimized away when used with ilog10_checked.
pub fn ilog10_mul_alt(x: u32) -> u32 {
    let guess = (x.ilog2() * 9) >> 5;
    let ttg = unsafe { *TEN_THRESHOLDS.get_unchecked(guess as usize) };
    guess + (x > ttg) as u32
}

const U64_THRESHOLDS: [u64; 19] = [
    9,
    99,
    999,
    9999,
    99999,
    999999,
    9999999,
    99999999,
    999999999,
    9999999999,
    99999999999,
    999999999999,
    9999999999999,
    99999999999999,
    999999999999999,
    9999999999999999,
    99999999999999999,
    999999999999999999,
    9999999999999999999,
];

pub fn ilog10_u64_mul(x: u64) -> u32 {
    // Use slightly more accurate approximation of log2(10) for u64;
    // this takes two lea instructions on x64 instead of just 1 but not bad.
    let guess: u32 = x.ilog2().wrapping_mul(19) >> 6;
    let ttg = unsafe { *U64_THRESHOLDS.get_unchecked(guess as usize) };
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
