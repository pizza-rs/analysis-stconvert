//! High-performance Simplified/Traditional Chinese converter.
//!
//! Performance design:
//! - **Fast ASCII bypass**: scans bytes directly; ASCII spans are copied
//!   without any char decoding or dictionary lookup.
//! - **Range guard**: chars outside the dictionary's codepoint range skip
//!   lookup instantly (single comparison).
//! - **Unified lookup**: one binary search gives both the single-char mapping
//!   and the multi_start flag (replaces two binary searches).
//! - **Stack buffer**: multi-char key accumulation uses a fixed 48-byte
//!   stack-resident buffer (no heap allocation).
//! - **`convert_to()` API**: callers can reuse an output buffer across calls.

use alloc::borrow::Cow;

use crate::config::ConvertType;
use crate::dict::Tables;
use crate::dict::UnifiedLookup;
use crate::dict::{self};

/// The converter. Holds pre-resolved table references for the configured
/// direction to avoid re-dispatching on every call.
#[derive(Clone, Copy)]
pub struct STConverter {
    tables: Tables,
}

impl STConverter {
    /// Create a converter for the given direction.
    #[inline]
    pub fn new(convert_type: ConvertType) -> Self {
        Self {
            tables: dict::tables_for(convert_type),
        }
    }

    /// Convert `input` and return a new `String`.
    #[inline]
    pub fn convert(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        self.convert_to(input, &mut out);
        out
    }

    /// Convert `input`, appending the result to `out`.
    ///
    /// This is the preferred API for high-throughput code: reuse the same
    /// `String` buffer across calls to amortize allocation.
    pub fn convert_to(&self, input: &str, out: &mut String) {
        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // FAST PATH: bulk-copy ASCII runs without char decoding.
            let ascii_start = i;
            while i < len && bytes[i] < 0x80 {
                i += 1;
            }
            if i > ascii_start {
                out.push_str(&input[ascii_start..i]);
                continue;
            }

            // Decode one non-ASCII char.
            let ch = match input[i..].chars().next() {
                Some(c) => c,
                None => break,
            };
            let char_len = ch.len_utf8();

            // Range guard + unified lookup (one binary search).
            match dict::lookup_unified(&self.tables, ch) {
                None => {
                    // Char not in dict range — pass through.
                    out.push_str(&input[i..i + char_len]);
                    i += char_len;
                }
                Some(UnifiedLookup {
                    mapped,
                    multi_start: false,
                }) => {
                    // Single-char mapping only, no multi-char possibility.
                    match mapped {
                        Some(c) => out.push(c),
                        None => out.push_str(&input[i..i + char_len]),
                    }
                    i += char_len;
                }
                Some(UnifiedLookup {
                    mapped,
                    multi_start: true,
                }) => {
                    // This char might start a multi-char key. Use greedy matching.
                    i += char_len;
                    self.greedy_match(input, &mut i, ch, mapped, out);
                }
            }
        }
    }

    /// Greedy longest-match for multi-char keys using partitioned groups + bloom filter.
    /// `first_char` is the char that triggered this path; `pos` points past it.
    #[inline(always)]
    fn greedy_match(
        &self,
        input: &str,
        pos: &mut usize,
        first_char: char,
        single_mapped: Option<char>,
        out: &mut String,
    ) {
        let tables = &self.tables;

        // Get the partitioned group for this first char (small slice, typically 2-5 entries).
        let group = dict::get_multi_group(tables, first_char);
        if group.is_empty() {
            // No multi-char entries for this first char — emit single mapping or original.
            match single_mapped {
                Some(c) => out.push(c),
                None => out.push(first_char),
            }
            return;
        }

        let mut buf = StackBuf::new();
        buf.push_char(first_char);

        let mut last_match: Option<MatchResult> = None;
        let mut consumed_bytes = first_char.len_utf8();
        let mut matched_bytes = 0usize;

        // Baseline: single-char mapping (if any).
        if let Some(c) = single_mapped {
            last_match = Some(MatchResult::Single(c));
            matched_bytes = consumed_bytes;
        }

        // Check if the first char alone is a multi-char key.
        if let Some(val) = dict::lookup_multi_in_group(group, buf.as_str()) {
            last_match = Some(MatchResult::Multi(val));
            matched_bytes = consumed_bytes;
        }

        // Extend while bloom filter says current buffer could be a prefix.
        // Bloom: O(1) with 2 L1-resident bit probes (16KB fits in L1 cache).
        let mut char_count = 1usize;
        while char_count < tables.max_key_len {
            if !dict::bloom_check(tables.prefix_bloom, dict::fx_hash(buf.as_str().as_bytes())) {
                break;
            }
            // Peek next char from input.
            let remaining = &input[*pos..];
            match remaining.chars().next() {
                Some(next_ch) => {
                    let next_len = next_ch.len_utf8();
                    buf.push_char(next_ch);
                    consumed_bytes += next_len;
                    *pos += next_len;
                    char_count += 1;

                    // Search within group (2-5 entries, 1-2 comparisons).
                    if let Some(val) = dict::lookup_multi_in_group(group, buf.as_str()) {
                        last_match = Some(MatchResult::Multi(val));
                        matched_bytes = consumed_bytes;
                    }
                }
                None => break,
            }
        }

        // Emit best match.
        if matched_bytes > 0 {
            match last_match.unwrap() {
                MatchResult::Single(c) => out.push(c),
                MatchResult::Multi(s) => out.push_str(s),
            }
            // Rewind: put back unmatched bytes for re-processing.
            if matched_bytes < consumed_bytes {
                *pos -= consumed_bytes - matched_bytes;
            }
        } else {
            // No match — emit first char as-is, rewind the rest.
            out.push(first_char);
            *pos -= consumed_bytes - first_char.len_utf8();
        }
    }

    /// Convert and return a `Cow` — borrows from input when no changes needed.
    #[inline]
    pub fn convert_cow<'a>(&self, input: &'a str) -> Cow<'a, str> {
        // Fast scan: does any char have a mapping or start a multi-key?
        let tables = &self.tables;
        let needs_conversion = input
            .chars()
            .any(|ch| match dict::lookup_unified(tables, ch) {
                None => false,
                Some(ul) => ul.mapped.is_some() || ul.multi_start,
            });
        if !needs_conversion {
            return Cow::Borrowed(input);
        }
        Cow::Owned(self.convert(input))
    }
}

/// Simple one-shot conversion function.
///
/// ```
/// use pizza_analysis_stconvert::convert;
/// use pizza_analysis_stconvert::ConvertType;
///
/// let result = convert("計算機科學與技術", ConvertType::T2S);
/// assert_eq!(result, "计算机科学与技术");
/// ```
#[inline]
pub fn convert(input: &str, convert_type: ConvertType) -> String {
    STConverter::new(convert_type).convert(input)
}

/// One-shot conversion appending to an existing buffer.
#[inline]
pub fn convert_to(input: &str, convert_type: ConvertType, out: &mut String) {
    STConverter::new(convert_type).convert_to(input, out);
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

enum MatchResult {
    Single(char),
    Multi(&'static str),
}

/// Stack-resident UTF-8 buffer for multi-char key accumulation.
/// 48 bytes covers all known dictionary keys (max ~12 chars × 4 bytes).
struct StackBuf {
    buf: [u8; 48],
    len: usize,
}

impl StackBuf {
    #[inline]
    fn new() -> Self {
        Self {
            buf: [0u8; 48],
            len: 0,
        }
    }

    #[inline]
    fn push_char(&mut self, c: char) {
        let encoded_len = c.len_utf8();
        debug_assert!(self.len + encoded_len <= 48);
        c.encode_utf8(&mut self.buf[self.len..]);
        self.len += encoded_len;
    }

    #[inline]
    fn as_str(&self) -> &str {
        // SAFETY: we only push valid UTF-8 chars.
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}
