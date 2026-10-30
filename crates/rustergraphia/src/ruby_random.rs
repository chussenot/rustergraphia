//! Ruby's `Random`, replicated: MT19937 with MRI's seeding and draw
//! algorithms, so that everything the Ruby library derives from a seed (a
//! plate's own tablet, the hand's tremor, the day's chapter) comes out the
//! same here, bit for bit.
//!
//! What is replicated, and from where in MRI (random.c, array.c):
//! - Seeding: the seed's absolute value packed into 32-bit words, least
//!   significant first; one word seeds `init_genrand`, more seed
//!   `init_by_array` (with MRI's guard: a top word of exactly 1 is dropped)
//! - `rand(n)` on integers: `limited_rand`, a mask-and-retry draw
//! - `rand` on floats: two 32-bit draws into a 53-bit mantissa, exclusive
//! - `rand(lo..hi)` inclusive ranges, integer and float (the float range
//!   interpolates `r*hi + (1-r)*lo` from an inclusive 53-bit real)
//! - `Array#shuffle(random:)` and `Array#sample(n, random:)` for n <= 3
//!   (all the library uses)

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// Ruby's `Random`: a seeded Mersenne Twister with MRI's exact draw methods.
#[derive(Clone)]
pub struct RubyRandom {
    state: [u32; N],
    index: usize,
}

impl std::fmt::Debug for RubyRandom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RubyRandom").finish_non_exhaustive()
    }
}

impl RubyRandom {
    /// `Random.new(seed)` for a seed that fits a signed 64-bit integer.
    /// Ruby seeds with the absolute value.
    pub fn new(seed: i64) -> Self {
        let magnitude = seed.unsigned_abs();
        let words = [magnitude as u32, (magnitude >> 32) as u32];
        if words[1] == 0 {
            Self::init_genrand(words[0])
        } else {
            Self::from_words(&words)
        }
    }

    /// `Random.new(seed)` for a big seed given as its magnitude's 32-bit
    /// words, least significant first (as `rb_integer_pack` lays them out).
    pub fn from_words(words: &[u32]) -> Self {
        let mut len = words.len();
        while len > 1 && words[len - 1] == 0 {
            len -= 1;
        }
        if len <= 1 {
            return Self::init_genrand(words.first().copied().unwrap_or(0));
        }
        // MRI's guard in rand_mt_init: a top word of exactly 1 is dropped
        if words[len - 1] == 1 {
            len -= 1;
        }
        Self::init_by_array(&words[..len])
    }

    /// `Random.new(seed)` for a seed given as the magnitude's big-endian
    /// bytes (a SHA-256 digest, say), as Ruby reads `hexdigest.to_i(16)`.
    pub fn from_be_bytes(bytes: &[u8]) -> Self {
        let mut padded = Vec::with_capacity(bytes.len() + 3);
        padded.resize((4 - bytes.len() % 4) % 4, 0u8);
        padded.extend_from_slice(bytes);
        let mut words: Vec<u32> = padded
            .chunks_exact(4)
            .map(|chunk| u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        words.reverse(); // least significant word first
        Self::from_words(&words)
    }

    /// A `Random` seeded from the operating system, where Ruby would use the
    /// process's own (no cross-implementation compatibility is needed there).
    pub fn new_os() -> Self {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes).expect("operating system randomness");
        Self::from_be_bytes(&bytes)
    }

    fn init_genrand(seed: u32) -> Self {
        let mut state = [0u32; N];
        state[0] = seed;
        for i in 1..N {
            state[i] = 1_812_433_253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        RubyRandom { state, index: N }
    }

    fn init_by_array(key: &[u32]) -> Self {
        let mut rng = Self::init_genrand(19_650_218);
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = N.max(key.len());
        while k > 0 {
            rng.state[i] = (rng.state[i]
                ^ (rng.state[i - 1] ^ (rng.state[i - 1] >> 30)).wrapping_mul(1_664_525))
            .wrapping_add(key[j])
            .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                rng.state[0] = rng.state[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            rng.state[i] = (rng.state[i]
                ^ (rng.state[i - 1] ^ (rng.state[i - 1] >> 30)).wrapping_mul(1_566_083_941))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                rng.state[0] = rng.state[N - 1];
                i = 1;
            }
            k -= 1;
        }
        rng.state[0] = 0x8000_0000;
        rng.index = N;
        rng
    }

    /// One tempered 32-bit output.
    pub fn genrand_int32(&mut self) -> u32 {
        if self.index >= N {
            for i in 0..N {
                let y = (self.state[i] & UPPER_MASK) | (self.state[(i + 1) % N] & LOWER_MASK);
                let mut next = self.state[(i + M) % N] ^ (y >> 1);
                if y & 1 != 0 {
                    next ^= MATRIX_A;
                }
                self.state[i] = next;
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// MRI's `limited_rand`: a value in `0..=limit`, by mask and retry.
    fn limited_rand(&mut self, limit: u64) -> u64 {
        if limit == 0 {
            return 0;
        }
        let mask = make_mask(limit);
        'retry: loop {
            let mut val: u64 = 0;
            for i in (0..=1u32).rev() {
                if (mask >> (i * 32)) & 0xffff_ffff != 0 {
                    val |= u64::from(self.genrand_int32()) << (i * 32);
                    val &= mask;
                    if limit < val {
                        continue 'retry;
                    }
                }
            }
            return val;
        }
    }

    /// `random.rand(n)` for a positive integer: a value in `0..n`.
    pub fn rand_below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "rand needs a positive bound");
        self.limited_rand(n - 1)
    }

    /// `random.rand(lo..hi)` for integers (inclusive on both ends).
    pub fn rand_int_incl(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo) as u64;
        lo + self.limited_rand(span) as i64
    }

    /// `random.rand`: a float in `[0, 1)`, 53 bits from two draws.
    pub fn rand_float(&mut self) -> f64 {
        let a = self.genrand_int32() >> 5;
        let b = self.genrand_int32() >> 6;
        (f64::from(a) * 67_108_864.0 + f64::from(b)) / 9_007_199_254_740_992.0
    }

    /// MRI's inclusive real in `[0, 1]`: a 64-bit draw scaled onto 2^53 + 1
    /// evenly spaced values.
    fn rand_real_inclusive(&mut self) -> f64 {
        let a = u64::from(self.genrand_int32());
        let b = u64::from(self.genrand_int32());
        let x = ((a << 32) | b) as u128;
        let m: u128 = (1u128 << 53) | 1;
        let r = ((x * m) >> 64) as u64;
        r as f64 / 9_007_199_254_740_992.0
    }

    /// `random.rand(lo..hi)` for floats (inclusive): MRI scales the
    /// inclusive real onto the range as `lo + r*(hi-lo)`.
    pub fn rand_float_incl(&mut self, lo: f64, hi: f64) -> f64 {
        let r = self.rand_real_inclusive();
        lo + r * (hi - lo)
    }

    /// `random.bytes(n)`: each 32-bit draw written little-endian.
    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let word = self.genrand_int32().to_le_bytes();
            let take = (n - out.len()).min(4);
            out.extend_from_slice(&word[..take]);
        }
        out
    }

    /// `array.shuffle(random: self)`, in place: MRI's Fisher-Yates.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        let mut i = items.len();
        while i > 0 {
            let j = self.rand_below(i as u64) as usize;
            i -= 1;
            items.swap(i, j);
        }
    }

    /// `array.sample(random: self)`: the index drawn for a single sample.
    pub fn sample_index(&mut self, len: usize) -> usize {
        self.rand_below(len as u64) as usize
    }

    /// `array.sample(n, random: self)`: the indices MRI draws, in MRI's
    /// order, for n of `len` (n <= len). Replicates array.c's special cases
    /// for n of 1 to 3, its sorted-insertion draw for n up to 10, and its
    /// partial shuffle beyond.
    pub fn sample_indices(&mut self, n: usize, len: usize) -> Vec<usize> {
        assert!(n <= len, "sample takes at most the whole array");
        match n {
            0 => Vec::new(),
            1 => vec![self.rand_below(len as u64) as usize],
            2 => {
                let i = self.rand_below(len as u64) as usize;
                let mut j = self.rand_below(len as u64 - 1) as usize;
                if j >= i {
                    j += 1;
                }
                vec![i, j]
            }
            3 => {
                let i = self.rand_below(len as u64) as usize;
                let mut j = self.rand_below(len as u64 - 1) as usize;
                let mut k = self.rand_below(len as u64 - 2) as usize;
                let l = if j >= i {
                    let l = i;
                    j += 1;
                    l
                } else {
                    j
                };
                let g = if j >= i { j } else { i };
                if k >= l {
                    k += 1;
                    if k >= g {
                        k += 1;
                    }
                }
                vec![i, j, k]
            }
            4..=10 => {
                // MRI draws into a shrinking range and shifts each draw past
                // the indices already taken (array.c's sorted-insertion case)
                let mut ids = Vec::with_capacity(n);
                let mut sorted: Vec<usize> = Vec::with_capacity(n);
                for i in 0..n {
                    let mut j = self.rand_below((len - i) as u64) as usize;
                    let mut at = 0;
                    while at < sorted.len() && j >= sorted[at] {
                        j += 1;
                        at += 1;
                    }
                    sorted.insert(at, j);
                    ids.push(j);
                }
                ids
            }
            _ => {
                // MRI copies the array and partially Fisher-Yates shuffles it
                let mut indices: Vec<usize> = (0..len).collect();
                for i in 0..n {
                    let j = self.rand_below((len - i) as u64) as usize + i;
                    indices.swap(i, j);
                }
                indices.truncate(n);
                indices
            }
        }
    }
}

/// The smallest `2^k - 1` that covers `x`.
fn make_mask(mut x: u64) -> u64 {
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x |= x >> 32;
    x
}
