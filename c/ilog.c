#include <stdio.h>
#include <stdint.h>
#include <inttypes.h>
#include <sys/time.h>
#include <assert.h>

inline uint32_t int_log2(uint32_t x) {
    return 31 - __builtin_clz(x);
}
const uint32_t table[] = {9, 99, 999, 9999, 99999,
        999999, 9999999, 99999999, 999999999, UINT32_MAX};

#if defined(__x86_64__)
inline uint32_t ilog10_asm(uint32_t x) {
    uint32_t dst;
    asm("lzcnt %1, %0\n\t"
        "xor $31, %0\n\t"
	"lea (%0, %0, 8), %0\n\t"
	"shr $5, %0\n\t"
	"cmp %1, table(, %0, 4)\n\t"
	"adc $0, %0\n\t"
	: "=r" (dst)
	: "r" (x)
	: "cc");
    return dst;
}

inline uint32_t ilog10_bsr(uint32_t x) {
    uint32_t dst;
    asm("bsr %1, %0\n\t"
	"lea (%0, %0, 8), %0\n\t"
	"shr $5, %0\n\t"
	"cmp %1, table(, %0, 4)\n\t"
	"adc $0, %0\n\t"
	: "=r" (dst)
	: "r" (x)
	: "cc");
    return dst;
}

static inline uint32_t mybsr(uint32_t x) __attribute__((__always_inline__));
static inline uint32_t mybsr(uint32_t x) {
	uint32_t result;
	asm(" bsr %1, %0\n"
			: "=r" (result)
			: "r"(x)
	   );
	return result;
}
#endif



uint32_t ilog10_warren(uint32_t x) {
    const uint32_t l2 = int_log2(x);
    const uint32_t y = (9 * l2) >> 5;
    //assert(y <= 9);
    uint32_t res = y + (x > table[y]);
    //printf("log2 %u is %u and l2 is %u and y is %u\n", x, res, l2, y);
    return res;
}

inline uint32_t ilogmap(uint32_t val_lz) {
    static const uint32_t LZ_GUESSMASK = 0b01001001000100100100010010010000;
    return __builtin_popcount(LZ_GUESSMASK << val_lz);
}

uint32_t ilog10_dga(uint32_t val) {
    uint32_t guess = ilogmap(__builtin_clz(val));
    uint32_t ttg = table[guess];
    return guess + (val > ttg);
}

uint32_t ilog10_quaternic(uint32_t x) {
    // uint32_t log2 = mybsr((x | 7) >> 1);
    const uint32_t log2 = 31 - __builtin_clz((x | 7) >> 1);
    const uint32_t guess = (log2 * 5) >> 4;
    static const uint32_t TEN_THRESHOLDS[] = {
        9,
        99,
        999,
        9999,
        99999,
        999999,
        9999999,
        99999999,
        999999999,
        UINT32_MAX
    };
    uint32_t ttg = TEN_THRESHOLDS[guess];
    return (guess + (x > ttg));
}

uint32_t ilog10_willet(uint32_t x) {
  static uint64_t table[] = {
      4294967296,  8589934582,  8589934582,  8589934582,  12884901788,
      12884901788, 12884901788, 17179868184, 17179868184, 17179868184,
      21474826480, 21474826480, 21474826480, 21474826480, 25769703776,
      25769703776, 25769703776, 30063771072, 30063771072, 30063771072,
      34349738368, 34349738368, 34349738368, 34349738368, 38554705664,
      38554705664, 38554705664, 41949672960, 41949672960, 41949672960,
      42949672960, 42949672960};
  return ((x + table[int_log2(x)]) >> 32);
}

void validate_ilog() {
    for (uint32_t i = 1; i < UINT32_MAX; i++) {
        uint32_t l1 = ilog10_dga(i);
        uint32_t l2 = ilog10_warren(i);
        if (l1 != l2) {
            printf("Eek mismatch %u:  %u vs %u\n", i, l1, l2);
        }
#if defined(__x86_64__)
        uint32_t l3 = ilog10_asm(i);
	if (l2 != l3) {
            printf("Eek mismatch %u:  %u vs %u\n", i, l1, l3);
	}
#endif
    }
}

#define TIME(name, x) do { \
    struct timeval tv_start, tv_end; \
    gettimeofday(&tv_start, NULL); \
    uint32_t r = x(); \
    gettimeofday(&tv_end, NULL); \
    if (r == 0) { \
        printf("Eek"); \
    } \
    uint64_t start = tv_start.tv_sec * 1000000 + tv_start.tv_usec; \
    uint64_t end = tv_end.tv_sec * 1000000 + tv_end.tv_usec; \
    printf("%s: %" PRIu64 "\n", name, end - start); \
} while (0)

#define BENCH_FUNC(n) \
    uint32_t bench_##n() { \
        uint32_t r = 0; \
        for (uint32_t i = 1; i < UINT32_MAX; i++) { \
            r ^= n(i); \
        } \
        return r; \
    }

BENCH_FUNC(ilog10_dga)

BENCH_FUNC(ilog10_warren)

BENCH_FUNC(ilog10_willet)

BENCH_FUNC(ilog10_quaternic)

#if defined(__x86_64__)
BENCH_FUNC(ilog10_asm)

BENCH_FUNC(ilog10_bsr)
#endif

void bench_ilog() {
    TIME("ilog2_dga", bench_ilog10_dga);
    TIME("ilog2_warren", bench_ilog10_warren);
    TIME("ilog2_willet", bench_ilog10_willet);
    TIME("ilog2_quaternic", bench_ilog10_quaternic);
#if defined(__x86_64__)
    TIME("ilog2_asm", bench_ilog10_asm);
    TIME("ilog2_bsr", bench_ilog10_bsr);
#endif
}

int main() { 
    //validate_ilog();
    bench_ilog();
}
