//! Optimized static dictionary lookup layer.
//!
//! Layout:
//! - **Unified table** `&[(u32, u32)]`: merges single-char mapping + multi_start
//!   flag into one sorted array. One binary search replaces two.
//! - **Multi-group index** `&[(u32, u32, u32)]`: sorted by first char codepoint.
//!   Partitions the multi table for O(log N/1808) group-local search.
//! - **Range guard**: chars outside `[MIN_KEY, MAX_KEY]` skip lookup entirely.

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

use crate::config::ConvertType;

/// Bit flag in unified table value indicating this char starts a multi-char key.
const MULTI_START_BIT: u32 = 1 << 31;
/// Mask to extract the mapped char codepoint (bits 0-20).
const CHAR_MASK: u32 = 0x1F_FFFF;

/// Tables bundle for a single conversion direction.
#[derive(Clone, Copy)]
pub(crate) struct Tables {
    /// Sorted unified table: (key_codepoint, packed_value).
    pub unified: &'static [(u32, u32)],
    /// Min/max key codepoint for range guard.
    pub min_key: u32,
    pub max_key: u32,
    /// Multi-char mappings, sorted by key for binary search.
    pub multi: &'static [(&'static str, &'static str)],
    /// Multi-group index: sorted by first_char codepoint.
    /// Each entry: (first_char_cp, start_in_multi, count).
    pub multi_groups: &'static [(u32, u32, u32)],
    /// Bloom filter (16KB) for O(1) prefix check. Fits in L1 cache.
    pub prefix_bloom: &'static [u64],
    /// Max char count of any multi-char key.
    pub max_key_len: usize,
}

/// Get the lookup tables for a conversion direction.
#[inline]
pub(crate) fn tables_for(ct: ConvertType) -> Tables {
    match ct {
        ConvertType::T2S => Tables {
            unified: T2S_UNIFIED,
            min_key: T2S_MIN_KEY,
            max_key: T2S_MAX_KEY,
            multi: T2S_MULTI,
            multi_groups: T2S_MULTI_GROUPS,
            prefix_bloom: T2S_PREFIX_BLOOM,
            max_key_len: T2S_MAX_KEY_LEN,
        },
        ConvertType::S2T => Tables {
            unified: S2T_UNIFIED,
            min_key: S2T_MIN_KEY,
            max_key: S2T_MAX_KEY,
            multi: S2T_MULTI,
            multi_groups: S2T_MULTI_GROUPS,
            prefix_bloom: S2T_PREFIX_BLOOM,
            max_key_len: S2T_MAX_KEY_LEN,
        },
        ConvertType::TW2CN => Tables {
            unified: TW2CN_UNIFIED,
            min_key: TW2CN_MIN_KEY,
            max_key: TW2CN_MAX_KEY,
            multi: TW2CN_MULTI,
            multi_groups: TW2CN_MULTI_GROUPS,
            prefix_bloom: TW2CN_PREFIX_BLOOM,
            max_key_len: TW2CN_MAX_KEY_LEN,
        },
        ConvertType::CN2TW => Tables {
            unified: CN2TW_UNIFIED,
            min_key: CN2TW_MIN_KEY,
            max_key: CN2TW_MAX_KEY,
            multi: CN2TW_MULTI,
            multi_groups: CN2TW_MULTI_GROUPS,
            prefix_bloom: CN2TW_PREFIX_BLOOM,
            max_key_len: CN2TW_MAX_KEY_LEN,
        },
        ConvertType::T2HK => Tables {
            unified: T2HK_UNIFIED,
            min_key: T2HK_MIN_KEY,
            max_key: T2HK_MAX_KEY,
            multi: T2HK_MULTI,
            multi_groups: T2HK_MULTI_GROUPS,
            prefix_bloom: T2HK_PREFIX_BLOOM,
            max_key_len: T2HK_MAX_KEY_LEN,
        },
        ConvertType::HK2T => Tables {
            unified: HK2T_UNIFIED,
            min_key: HK2T_MIN_KEY,
            max_key: HK2T_MAX_KEY,
            multi: HK2T_MULTI,
            multi_groups: HK2T_MULTI_GROUPS,
            prefix_bloom: HK2T_PREFIX_BLOOM,
            max_key_len: HK2T_MAX_KEY_LEN,
        },
        ConvertType::JP2T => Tables {
            unified: JP_UNIFIED,
            min_key: JP_MIN_KEY,
            max_key: JP_MAX_KEY,
            multi: JP_MULTI,
            multi_groups: JP_MULTI_GROUPS,
            prefix_bloom: JP_PREFIX_BLOOM,
            max_key_len: JP_MAX_KEY_LEN,
        },
    }
}

/// Result of looking up a char in the unified table.
#[derive(Clone, Copy)]
pub(crate) struct UnifiedLookup {
    /// Mapped char (None if no single-char mapping exists).
    pub mapped: Option<char>,
    /// Whether this char starts a multi-char key.
    pub multi_start: bool,
}

/// Single binary search in the unified table — replaces two lookups.
/// Returns None if char is outside the table's range.
#[inline]
pub(crate) fn lookup_unified(tables: &Tables, c: char) -> Option<UnifiedLookup> {
    let cp = c as u32;
    // Range guard: instant skip for ASCII and out-of-range chars.
    if cp < tables.min_key || cp > tables.max_key {
        return None;
    }
    match tables.unified.binary_search_by_key(&cp, |&(k, _)| k) {
        Ok(i) => {
            let val = tables.unified[i].1;
            let mapped_cp = val & CHAR_MASK;
            let mapped = if mapped_cp != 0 {
                // SAFETY: mapped_cp is a valid Unicode codepoint from our dict.
                Some(unsafe { char::from_u32_unchecked(mapped_cp) })
            } else {
                None
            };
            let multi_start = (val & MULTI_START_BIT) != 0;
            Some(UnifiedLookup { mapped, multi_start })
        }
        Err(_) => None,
    }
}

/// Find the multi-char group for a given first char.
/// Returns the slice of multi entries starting with this char.
#[inline]
pub(crate) fn get_multi_group(tables: &Tables, first_char: char) -> &'static [(&'static str, &'static str)] {
    let cp = first_char as u32;
    match tables.multi_groups.binary_search_by_key(&cp, |&(k, _, _)| k) {
        Ok(i) => {
            let (_, start, count) = tables.multi_groups[i];
            &tables.multi[start as usize..(start + count) as usize]
        }
        Err(_) => &[],
    }
}

/// Search for an exact multi-char key within a group slice.
#[inline]
pub(crate) fn lookup_multi_in_group(
    group: &[(&'static str, &'static str)],
    key: &str,
) -> Option<&'static str> {
    group
        .binary_search_by_key(&key, |&(k, _)| k)
        .ok()
        .map(|i| group[i].1)
}

/// FxHash-64 seed constant.
const FX_SEED: u64 = 0x517cc1b727220a95;

/// FxHash-64: compute hash of a byte slice.
#[inline]
pub(crate) fn fx_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0;
    for &b in bytes {
        hash = (hash.rotate_left(5) ^ (b as u64)).wrapping_mul(FX_SEED);
    }
    hash
}

/// Bloom filter size: 2^17 = 131072 bits = 16KB.
const BLOOM_BITS: usize = 131072;

/// Check if a precomputed hash matches the bloom filter.
/// O(1): two bit probes in a 16KB array that fits in L1 cache.
#[inline]
pub(crate) fn bloom_check(bloom: &[u64], hash: u64) -> bool {
    let idx1 = (hash as usize) & (BLOOM_BITS - 1);
    let idx2 = ((hash >> 17) as usize) & (BLOOM_BITS - 1);
    let w1 = unsafe { *bloom.get_unchecked(idx1 / 64) };
    let w2 = unsafe { *bloom.get_unchecked(idx2 / 64) };
    (w1 & (1u64 << (idx1 % 64))) != 0 && (w2 & (1u64 << (idx2 % 64))) != 0
}
