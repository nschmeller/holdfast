//! A tiny deterministic PRNG.
//!
//! We roll our own rather than pulling in `rand`, which drags `getrandom` and a
//! wasm-specific JS shim behind it. xoshiro256++ is small, fast and has better
//! statistical quality than the xorshift variants usually inlined for this.

use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Default for Rng {
    fn default() -> Self {
        Self::seeded(0x5DE_5C_0FFEE)
    }
}

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        // SplitMix64 to spread a single seed across the full state.
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut next = || {
            z = z.wrapping_add(0x9E3779B97F4A7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
            x ^ (x >> 31)
        };
        Self {
            s: [next(), next(), next(), next()],
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn f32(&mut self) -> f32 {
        // Take the top 24 bits: exactly the mantissa width of f32, so every
        // value is representable and the distribution stays uniform.
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in `[min, max)`.
    #[inline]
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.f32() * (max - min)
    }

    /// Uniform integer in `[0, n)`. Returns 0 when `n == 0`.
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// A random point on the unit circle, in the XZ plane.
    #[inline]
    pub fn unit_circle(&mut self) -> Vec3 {
        let a = self.range(0.0, std::f32::consts::TAU);
        Vec3::new(a.cos(), 0.0, a.sin())
    }

    /// A random point inside a disc of `radius`, in the XZ plane.
    #[inline]
    pub fn in_disc(&mut self, radius: f32) -> Vec3 {
        // sqrt keeps the samples area-uniform instead of clumping at the centre.
        self.unit_circle() * (radius * self.f32().sqrt())
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            Some(&items[self.below(items.len())])
        }
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}
