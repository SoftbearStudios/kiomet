// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use kodiak_client::fxhash::FxHashMap;
use std::hash::Hash;
use std::ops::RangeInclusive;

type Word = u64;

fn bitmask(range: &RangeInclusive<u8>, i: usize) -> Word {
    fn less(i: usize) -> Word {
        if let Some(v) = 1u64.checked_shl(i as u32) {
            v - 1
        } else {
            Word::MAX
        }
    }

    let i = (i * Word::BITS as usize) as usize;
    let greater = !less((*range.start() as usize).saturating_sub(i));
    let less = less((*range.end() as usize + 1).saturating_sub(i));
    greater & less
}

/// A set with a u8 key.
#[derive(Default)]
struct FiniteBitset {
    bitset: [Word; 4],
}

impl FiniteBitset {
    /// Inserts the first free index in the set and range. Returns the index or `range.end()` if
    /// full.
    fn alloc(&mut self, range: &RangeInclusive<u8>) -> u8 {
        self.bitset
            .iter_mut()
            .enumerate()
            .filter_map(|(i, bitset)| {
                let masked_bitset = *bitset | !bitmask(range, i);
                (masked_bitset != Word::MAX).then_some((i, bitset, masked_bitset))
            })
            .next()
            .map(|(i, bitset, masked_bitset)| {
                let j = masked_bitset.trailing_ones();
                *bitset |= 1 << j;
                (j + i as u32 * Word::BITS) as u8
            })
            .unwrap_or(*range.end())
    }

    /// Removes an index from the set. Returns true if the index was removed (was present).
    fn remove(&mut self, index: u8) -> bool {
        let i = (index / Word::BITS as u8) as usize;
        let j = index % Word::BITS as u8;
        let bit = 1 << j;

        let previous = self.bitset[i];
        let removed = previous & bit != 0;
        self.bitset[i] = previous & !bit;
        removed
    }
}

struct FiniteArenaAllocation {
    index: u8,
    keepalive: u8,
}

/// Maps a key type `T` to a unique u8 index. Indices are chosen sequentially and are stable if
/// continuously used. Unused indices are cleared each on each call to [`FiniteArena::tick`]..
pub struct FiniteArena<T> {
    bitset: FiniteBitset,
    indices: FxHashMap<T, FiniteArenaAllocation>,
}

impl<T> Default for FiniteArena<T> {
    fn default() -> Self {
        Self {
            bitset: Default::default(),
            indices: Default::default(),
        }
    }
}

impl<T: Hash + Eq> FiniteArena<T> {
    /// Removes unused keys and forces specific keys to be specific indices.
    pub fn tick(&mut self) {
        self.indices.retain(|_, a| {
            if let Some(keepalive) = a.keepalive.checked_sub(1) {
                a.keepalive = keepalive;
                true
            } else {
                // Ignore if it was actually removed since 2 keys can have the same value if the bitset is full.
                self.bitset.remove(a.index);
                false
            }
        });
    }

    pub fn get(&mut self, key: T, mut f: impl FnMut() -> RangeInclusive<u8>) -> u8 {
        let keepalive = 1;
        let a = self
            .indices
            .entry(key)
            .or_insert_with(|| FiniteArenaAllocation {
                index: self.bitset.alloc(&f()),
                keepalive,
            });

        // Make sure the old index is in the new range. Only do this once per tick since calculating
        // the range isn't free.
        if a.keepalive == 0 {
            a.keepalive = keepalive;

            let index = a.index;
            let range = f();
            if !range.contains(&index) {
                self.bitset.remove(index);
                *a = FiniteArenaAllocation {
                    index: self.bitset.alloc(&range),
                    keepalive,
                };
            }
        }
        a.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finite_bitset() {
        let r = RangeInclusive::new(0, u8::MAX);
        let mut set = FiniteBitset::default();
        assert_eq!(set.alloc(&r), 0);
        assert_eq!(set.alloc(&r), 1);
        set.remove(0);
        assert_eq!(set.alloc(&r), 0);
        assert_eq!(set.alloc(&r), 2);

        let mut set = FiniteBitset::default();
        for i in 0..=u8::MAX {
            assert_eq!(set.alloc(&r), i);
        }
        for _ in 0..10 {
            assert_eq!(set.alloc(&r), u8::MAX);
        }

        let mut set = FiniteBitset::default();
        let r = RangeInclusive::new(1, 1);
        assert_eq!(set.alloc(&r), 1);
        assert_eq!(set.alloc(&r), 1);

        let r = RangeInclusive::new(1, 3);
        assert_eq!(set.alloc(&r), 2);
        assert_eq!(set.alloc(&r), 3);
        assert_eq!(set.alloc(&r), 3);
    }

    #[test]
    fn test_finite_arena() {
        let f = || RangeInclusive::new(0, u8::MAX);

        let mut arena = FiniteArena::default();
        assert_eq!(arena.get(0, f), 0);
        assert_eq!(arena.get(0, f), 0);
        assert_eq!(arena.get(1, f), 1);
        assert_eq!(arena.get(1, f), 1);

        arena.tick();
        assert_eq!(arena.get(1, f), 1);

        arena.tick();
        assert_eq!(arena.get(2, f), 0);

        arena.tick();
        assert_eq!(arena.get(0, f), 1);
        assert_eq!(arena.get(1, f), 2);

        assert_eq!(arena.get(0, f), 1);
        assert_eq!(arena.get(1, f), 2);
        assert_eq!(arena.get(2, f), 0);
        assert_eq!(arena.get(3, f), 3);
    }
}
