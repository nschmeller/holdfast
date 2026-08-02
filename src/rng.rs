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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_stream() {
        let mut a = Rng::seeded(42);
        let mut b = Rng::seeded(42);
        for _ in 0..256 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::seeded(1);
        let mut b = Rng::seeded(2);
        // Astronomically unlikely to collide on all of these by chance.
        let differs = (0..32).any(|_| a.next_u64() != b.next_u64());
        assert!(differs);
    }

    #[test]
    fn f32_stays_in_unit_interval() {
        let mut rng = Rng::seeded(7);
        for _ in 0..20_000 {
            let v = rng.f32();
            assert!((0.0..1.0).contains(&v), "{v} outside [0, 1)");
        }
    }

    #[test]
    fn f32_is_roughly_uniform() {
        // Ten buckets over 100k samples: each should land near 10%.
        let mut rng = Rng::seeded(99);
        let mut buckets = [0u32; 10];
        const N: u32 = 100_000;
        for _ in 0..N {
            buckets[(rng.f32() * 10.0) as usize % 10] += 1;
        }
        for (i, count) in buckets.iter().enumerate() {
            let share = f64::from(*count) / f64::from(N);
            assert!(
                (0.085..0.115).contains(&share),
                "bucket {i} had share {share}"
            );
        }
    }

    #[test]
    fn range_respects_bounds() {
        let mut rng = Rng::seeded(3);
        for _ in 0..10_000 {
            let v = rng.range(-3.5, 8.25);
            assert!((-3.5..8.25).contains(&v));
        }
    }

    #[test]
    fn range_with_equal_bounds_is_constant() {
        let mut rng = Rng::seeded(3);
        assert_eq!(rng.range(2.0, 2.0), 2.0);
    }

    #[test]
    fn below_is_bounded_and_handles_zero() {
        let mut rng = Rng::seeded(11);
        assert_eq!(rng.below(0), 0, "below(0) must not divide by zero");
        for n in 1..40 {
            for _ in 0..200 {
                assert!(rng.below(n) < n);
            }
        }
    }

    #[test]
    fn chance_honours_certainty() {
        let mut rng = Rng::seeded(5);
        for _ in 0..1000 {
            assert!(!rng.chance(0.0), "chance(0.0) must never fire");
            assert!(rng.chance(1.0), "chance(1.0) must always fire");
        }
    }

    #[test]
    fn chance_approximates_its_probability() {
        let mut rng = Rng::seeded(13);
        let hits = (0..50_000).filter(|_| rng.chance(0.25)).count();
        let share = hits as f64 / 50_000.0;
        assert!((0.24..0.26).contains(&share), "share was {share}");
    }

    #[test]
    fn unit_circle_is_unit_length_and_flat() {
        let mut rng = Rng::seeded(17);
        for _ in 0..1000 {
            let v = rng.unit_circle();
            assert!((v.length() - 1.0).abs() < 1e-4);
            assert_eq!(v.y, 0.0, "unit_circle must stay in the XZ plane");
        }
    }

    #[test]
    fn in_disc_stays_inside_radius() {
        let mut rng = Rng::seeded(19);
        for _ in 0..5000 {
            let v = rng.in_disc(3.0);
            assert!(v.length() <= 3.0 + 1e-4);
            assert_eq!(v.y, 0.0);
        }
    }

    #[test]
    fn in_disc_is_area_uniform() {
        // Half the radius encloses a quarter of the area, so about a quarter
        // of the samples should land inside it. A naive r = rand() would put
        // half of them there.
        let mut rng = Rng::seeded(23);
        const N: usize = 40_000;
        let inner = (0..N)
            .filter(|_| rng.in_disc(1.0).length() < 0.5)
            .count();
        let share = inner as f64 / N as f64;
        assert!((0.23..0.27).contains(&share), "share was {share}");
    }

    #[test]
    fn pick_returns_none_only_when_empty() {
        let mut rng = Rng::seeded(29);
        let empty: [u8; 0] = [];
        assert!(rng.pick(&empty).is_none());
        let items = [1, 2, 3];
        for _ in 0..100 {
            assert!(items.contains(rng.pick(&items).unwrap()));
        }
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut rng = Rng::seeded(31);
        let mut items: Vec<u32> = (0..64).collect();
        rng.shuffle(&mut items);
        let mut sorted = items.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_actually_reorders() {
        let mut rng = Rng::seeded(37);
        let original: Vec<u32> = (0..64).collect();
        let mut items = original.clone();
        rng.shuffle(&mut items);
        assert_ne!(items, original);
    }

    #[test]
    fn shuffle_handles_degenerate_lengths() {
        let mut rng = Rng::seeded(41);
        let mut empty: Vec<u32> = vec![];
        rng.shuffle(&mut empty);
        let mut single = vec![9];
        rng.shuffle(&mut single);
        assert_eq!(single, vec![9]);
    }
}
