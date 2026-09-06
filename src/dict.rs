//! Optimized static dictionary lookup layer.
//!
//! Layout:
//! - **Unified table** `&[(u32, u32)]`: merges single-char mapping + multi_start
//!   flag into one sorted array. One binary search replaces two.
//! - **Multi-group index** `&[(u32, u32, u32)]`: sorted by first char codepoint.
//!   Partitions the multi table for O(log N/1808) group-local search.
//! - **Range guard**: chars outside `[MIN_KEY, MAX_KEY]` skip lookup entirely.

#[cfg(feature = "embed")]
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(all(not(feature = "embed"), not(feature = "std")))]
compile_error!(
    "pizza-analysis-stconvert requires either the `embed` feature (compile-time \
     dictionaries) or the `std` feature (runtime dictionary loading)."
);

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
#[cfg(feature = "embed")]
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
            Some(UnifiedLookup {
                mapped,
                multi_start,
            })
        }
        Err(_) => None,
    }
}

/// Find the multi-char group for a given first char.
/// Returns the slice of multi entries starting with this char.
#[inline]
pub(crate) fn get_multi_group(
    tables: &Tables,
    first_char: char,
) -> &'static [(&'static str, &'static str)] {
    let cp = first_char as u32;
    match tables
        .multi_groups
        .binary_search_by_key(&cp, |&(k, _, _)| k)
    {
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

// ─── Runtime table construction (no-embed builds) ───────────────────────────
//
// When the `embed` feature is disabled we do not bake the optimized lookup
// tables at compile time. Instead they are built **once per direction** on first
// use from the external dictionary file (`config/analysis/stconvert/<name>`,
// when a dictionary directory is configured) or the embedded raw text, using the
// exact same layout the build script produces. The resulting tables are leaked
// to `'static` and cached, so the per-character hot path in `converter.rs` is
// byte-for-byte identical to the embedded build — only table construction moves
// from compile time to a one-off startup parse.

#[cfg(not(feature = "embed"))]
#[inline]
pub(crate) fn tables_for(ct: ConvertType) -> Tables {
    use std::sync::OnceLock;
    static CELLS: [OnceLock<Tables>; 7] = [const { OnceLock::new() }; 7];
    *CELLS[ct as usize].get_or_init(|| runtime::build_tables(ct as usize))
}

#[cfg(not(feature = "embed"))]
mod runtime {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::collections::BTreeSet;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::fx_hash;
    use super::Tables;

    const MULTI_START_BIT: u32 = 1 << 31;
    const BLOOM_BITS: usize = 131072;
    const BLOOM_WORDS: usize = BLOOM_BITS / 64;

    /// File names under the `stconvert` dictionary namespace, indexed by
    /// `ConvertType as usize`.
    const FILE_NAMES: [&str; 7] = [
        "t2s.txt",
        "s2t.txt",
        "tw2cn.txt",
        "cn2tw.txt",
        "t2hk.txt",
        "hk2t.txt",
        "jp_variants.txt",
    ];

    /// Embedded raw dictionaries — the single source of truth shared with the
    /// build script. Always present as a fallback so a missing external file
    /// degrades gracefully rather than failing.
    #[cfg(feature = "embed-fallback")]
    static EMBEDDED: [&str; 7] = [
        include_str!("../data/t2s.txt"),
        include_str!("../data/s2t.txt"),
        include_str!("../data/tw2cn.txt"),
        include_str!("../data/cn2tw.txt"),
        include_str!("../data/t2hk.txt"),
        include_str!("../data/hk2t.txt"),
        include_str!("../data/jp_variants.txt"),
    ];
    #[cfg(not(feature = "embed-fallback"))]
    static EMBEDDED: [&str; 7] = [""; 7];

    fn raw_text(idx: usize) -> Cow<'static, str> {
        let embedded = EMBEDDED[idx];
        #[cfg(all(feature = "engine", feature = "std"))]
        {
            pizza_engine::analysis::dict::load_str("stconvert", FILE_NAMES[idx], Some(embedded))
                .unwrap_or(Cow::Borrowed(embedded))
        }
        #[cfg(not(all(feature = "engine", feature = "std")))]
        {
            let _ = FILE_NAMES;
            Cow::Borrowed(embedded)
        }
    }

    /// Build the lookup tables for one direction, mirroring `build.rs`.
    pub(super) fn build_tables(idx: usize) -> Tables {
        let text = raw_text(idx);

        let mut single: Vec<(char, char)> = Vec::new();
        let mut multi: Vec<(String, String)> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let sep = match line.find(':') {
                Some(i) => i,
                None => continue,
            };
            let key = &line[..sep];
            let val = &line[sep + 1..];
            if key.is_empty() || val.is_empty() || key == val {
                continue;
            }
            let key_chars: Vec<char> = key.chars().collect();
            if key_chars.len() == 1 && val.chars().count() == 1 {
                single.push((key_chars[0], val.chars().next().unwrap()));
            } else {
                multi.push((String::from(key), String::from(val)));
            }
        }
        single.sort_by_key(|&(k, _)| k);
        single.dedup_by_key(|e| e.0);
        multi.sort_by(|a, b| a.0.cmp(&b.0));
        multi.dedup_by(|a, b| a.0 == b.0);

        // First chars of every multi-char key.
        let mut first_chars: BTreeSet<char> = BTreeSet::new();
        for (key, _) in &multi {
            if let Some(c) = key.chars().next() {
                first_chars.insert(c);
            }
        }

        // Unified table: single mappings + multi_start flags, sorted by key.
        let single_keys: BTreeSet<char> = single.iter().map(|&(k, _)| k).collect();
        let mut unified: Vec<(u32, u32)> = Vec::new();
        for &(k, v) in &single {
            let mut val = v as u32;
            if first_chars.contains(&k) {
                val |= MULTI_START_BIT;
            }
            unified.push((k as u32, val));
        }
        for &c in &first_chars {
            if !single_keys.contains(&c) {
                unified.push((c as u32, MULTI_START_BIT));
            }
        }
        unified.sort_by_key(|&(k, _)| k);
        let min_key = unified.first().map(|&(k, _)| k).unwrap_or(0);
        let max_key = unified.last().map(|&(k, _)| k).unwrap_or(0);

        // Multi-group index: (first_char_cp, start, count), contiguous since sorted.
        let mut groups: Vec<(u32, u32, u32)> = Vec::new();
        let mut i = 0;
        while i < multi.len() {
            let first_char = multi[i].0.chars().next().unwrap();
            let start = i;
            while i < multi.len() && multi[i].0.chars().next().unwrap() == first_char {
                i += 1;
            }
            groups.push((first_char as u32, start as u32, (i - start) as u32));
        }

        // Prefix bloom filter (same 16 KB layout as the build script).
        let mut prefixes: BTreeSet<String> = BTreeSet::new();
        for (key, _) in &multi {
            let chars: Vec<char> = key.chars().collect();
            for j in 1..chars.len() {
                prefixes.insert(chars[..j].iter().collect());
            }
        }
        let mut bloom = vec![0u64; BLOOM_WORDS];
        for p in &prefixes {
            let h = fx_hash(p.as_bytes());
            let idx1 = (h as usize) & (BLOOM_BITS - 1);
            let idx2 = ((h >> 17) as usize) & (BLOOM_BITS - 1);
            bloom[idx1 / 64] |= 1u64 << (idx1 % 64);
            bloom[idx2 / 64] |= 1u64 << (idx2 % 64);
        }

        let max_key_len = multi
            .iter()
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(1);

        // Leak owned data to `'static` (one-time, lives for the process).
        let multi_static: Vec<(&'static str, &'static str)> = multi
            .into_iter()
            .map(|(k, v)| {
                (
                    &*Box::leak(k.into_boxed_str()),
                    &*Box::leak(v.into_boxed_str()),
                )
            })
            .collect();

        Tables {
            unified: Box::leak(unified.into_boxed_slice()),
            min_key,
            max_key,
            multi: Box::leak(multi_static.into_boxed_slice()),
            multi_groups: Box::leak(groups.into_boxed_slice()),
            prefix_bloom: Box::leak(bloom.into_boxed_slice()),
            max_key_len,
        }
    }
}
