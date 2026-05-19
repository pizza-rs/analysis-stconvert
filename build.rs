//! Build script: generate optimized static lookup tables.
//!
//! Layout per direction:
//! - `{DIR}_UNIFIED`: sorted `&[(u32, u32)]` — merged single-char + multi_start
//!   lookup. Key = char codepoint. Value packs: bit 31 = is_multi_start,
//!   bits 0-20 = mapped char codepoint (0 = no single mapping).
//! - `{DIR}_MIN_KEY` / `{DIR}_MAX_KEY`: range guard constants.
//! - `{DIR}_MULTI`: sorted `&[(&str, &str)]` for multi-char mappings.
//! - `{DIR}_PREFIX_HASHES`: sorted `&[u64]` — FxHash of each prefix string.
//!   Enables O(1)-quality prefix check via hash + integer binary search.
//! - `{DIR}_MAX_KEY_LEN`: max char count of any multi-char key.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

struct Dict {
    single: Vec<(char, char)>,
    multi: Vec<(String, String)>,
}

const MULTI_START_BIT: u32 = 1 << 31;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("generated.rs");

    let data_files = [
        "data/t2s.txt",
        "data/s2t.txt",
        "data/tw2cn.txt",
        "data/cn2tw.txt",
        "data/t2hk.txt",
        "data/hk2t.txt",
        "data/jp_variants.txt",
    ];
    for f in &data_files {
        println!("cargo:rerun-if-changed={f}");
    }

    let mut out = fs::File::create(&dest).unwrap();

    let directions: &[(&str, &str)] = &[
        ("T2S", "data/t2s.txt"),
        ("S2T", "data/s2t.txt"),
        ("TW2CN", "data/tw2cn.txt"),
        ("CN2TW", "data/cn2tw.txt"),
        ("T2HK", "data/t2hk.txt"),
        ("HK2T", "data/hk2t.txt"),
        ("JP", "data/jp_variants.txt"),
    ];

    for &(prefix, path) in directions {
        let dict = parse_dict(Path::new(path));
        let first_chars = build_first_chars(&dict.multi);
        let prefixes = build_prefix_set(&dict.multi);
        let max_key_len = dict
            .multi
            .iter()
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(1);

        // Write unified single + multi_start table
        write_unified_table(&mut out, prefix, &dict.single, &first_chars);

        // Write packed multi-char blob + index
        write_packed_multi(&mut out, prefix, &dict.multi);

        // Write multi-group offsets for partitioned lookup
        write_multi_groups(&mut out, prefix, &dict.multi);

        // Write prefix bloom filter (16KB, fits in L1, replaces 112KB sorted hash array)
        write_prefix_bloom(&mut out, &format!("{prefix}_PREFIX_BLOOM"), &prefixes);

        writeln!(
            out,
            "pub(crate) const {prefix}_MAX_KEY_LEN: usize = {max_key_len};\n"
        )
        .unwrap();
    }
}

fn parse_dict(path: &Path) -> Dict {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

    let mut single: Vec<(char, char)> = Vec::new();
    let mut multi: Vec<(String, String)> = Vec::new();

    for line in content.lines() {
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
            multi.push((key.to_string(), val.to_string()));
        }
    }

    single.sort_by_key(|&(k, _)| k);
    single.dedup_by_key(|e| e.0);
    multi.sort_by(|a, b| a.0.cmp(&b.0));
    multi.dedup_by(|a, b| a.0 == b.0);

    Dict { single, multi }
}

fn build_first_chars(multi: &[(String, String)]) -> BTreeSet<char> {
    let mut chars: BTreeSet<char> = BTreeSet::new();
    for (key, _) in multi {
        if let Some(c) = key.chars().next() {
            chars.insert(c);
        }
    }
    chars
}

fn build_prefix_set(multi: &[(String, String)]) -> Vec<String> {
    let mut prefixes: BTreeSet<String> = BTreeSet::new();
    for (key, _) in multi {
        let chars: Vec<char> = key.chars().collect();
        for i in 1..chars.len() {
            prefixes.insert(chars[..i].iter().collect());
        }
    }
    prefixes.into_iter().collect()
}

/// Write unified table merging single-char mappings and multi_start flags.
/// Sorted by key (u32 codepoint). One binary search replaces two.
fn write_unified_table(
    out: &mut fs::File,
    prefix: &str,
    single: &[(char, char)],
    first_chars: &BTreeSet<char>,
) {
    // Merge: every char that has a single mapping OR starts a multi-char key.
    let mut unified: Vec<(u32, u32)> = Vec::new();

    // Add all single-char entries.
    for &(k, v) in single {
        let mut val = v as u32;
        if first_chars.contains(&k) {
            val |= MULTI_START_BIT;
        }
        unified.push((k as u32, val));
    }

    // Add first_chars that DON'T have a single mapping.
    let single_keys: BTreeSet<char> = single.iter().map(|&(k, _)| k).collect();
    for &c in first_chars {
        if !single_keys.contains(&c) {
            unified.push((c as u32, MULTI_START_BIT)); // no single mapping, just multi_start
        }
    }

    unified.sort_by_key(|&(k, _)| k);

    let min_key = unified.first().map(|&(k, _)| k).unwrap_or(0);
    let max_key = unified.last().map(|&(k, _)| k).unwrap_or(0);

    writeln!(out, "pub(crate) const {prefix}_MIN_KEY: u32 = 0x{min_key:X};").unwrap();
    writeln!(out, "pub(crate) const {prefix}_MAX_KEY: u32 = 0x{max_key:X};").unwrap();
    writeln!(out, "pub(crate) static {prefix}_UNIFIED: &[(u32, u32)] = &[").unwrap();
    for &(k, v) in &unified {
        writeln!(out, "    (0x{k:X}, 0x{v:X}),").unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

/// Write packed multi-char table: keep direct &[(&str, &str)] for fast lookup.
/// The binary search comparison is faster with pre-made string slices.
fn write_packed_multi(out: &mut fs::File, prefix: &str, multi: &[(String, String)]) {
    writeln!(out, "pub(crate) static {prefix}_MULTI: &[(&str, &str)] = &[").unwrap();
    for (k, v) in multi {
        writeln!(out, "    ({:?}, {:?}),", k, v).unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

/// Write multi-group offset table: sorted by first char codepoint.
/// Each entry: (first_char_codepoint: u32, packed: u32) where packed = (start << 16) | count.
/// Binary search on u32 keys gives the partition boundaries for greedy matching.
fn write_multi_groups(out: &mut fs::File, prefix: &str, multi: &[(String, String)]) {
    // Since multi is sorted by key, entries with the same first char are contiguous.
    let mut groups: Vec<(u32, u32, u32)> = Vec::new(); // (first_char_cp, start, count)
    let mut i = 0;
    while i < multi.len() {
        let first_char = multi[i].0.chars().next().unwrap();
        let start = i;
        while i < multi.len() && multi[i].0.chars().next().unwrap() == first_char {
            i += 1;
        }
        groups.push((first_char as u32, start as u32, (i - start) as u32));
    }

    writeln!(
        out,
        "pub(crate) static {prefix}_MULTI_GROUPS: &[(u32, u32, u32)] = &["
    )
    .unwrap();
    for &(cp, start, count) in &groups {
        writeln!(out, "    (0x{cp:X}, {start}, {count}),").unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

/// FxHash-64 for build-time prefix hashing. Same algorithm used at runtime.
fn fx_hash(bytes: &[u8]) -> u64 {
    const SEED: u64 = 0x517cc1b727220a95;
    let mut hash: u64 = 0;
    for &b in bytes {
        hash = (hash.rotate_left(5) ^ (b as u64)).wrapping_mul(SEED);
    }
    hash
}

/// Write prefix bloom filter: 16KB bit array for O(1) prefix check.
/// Two probes derived from a single FxHash, fits entirely in L1 cache.
fn write_prefix_bloom(out: &mut fs::File, name: &str, prefixes: &[String]) {
    const BLOOM_BITS: usize = 131072; // 2^17
    const BLOOM_WORDS: usize = BLOOM_BITS / 64; // 2048

    let mut bloom = vec![0u64; BLOOM_WORDS];
    for p in prefixes {
        let h = fx_hash(p.as_bytes());
        let idx1 = (h as usize) & (BLOOM_BITS - 1);
        let idx2 = ((h >> 17) as usize) & (BLOOM_BITS - 1);
        bloom[idx1 / 64] |= 1u64 << (idx1 % 64);
        bloom[idx2 / 64] |= 1u64 << (idx2 % 64);
    }

    writeln!(out, "pub(crate) static {name}: &[u64] = &[").unwrap();
    for w in &bloom {
        writeln!(out, "    0x{w:016X},").unwrap();
    }
    writeln!(out, "];\n").unwrap();
}
