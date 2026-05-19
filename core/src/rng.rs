/// Tiny seedable xorshift64 PRNG used by the region generators. The level
/// number is the seed.
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        // xorshift64 is broken at state 0; remap so seed 0 is still usable.
        let s = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state: s }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub(crate) fn range(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (self.next_u64() % n as u64) as usize
    }

    pub(crate) fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.range(i + 1);
            slice.swap(i, j);
        }
    }
}
