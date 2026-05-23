//! Comprehensive tests for pizza-analysis-stconvert (Simplified/Traditional Chinese conversion).

use pizza_analysis_stconvert::{convert, convert_to, ConvertConfig, ConvertType, STConverter};

// ═══════════════════════════════════════════════════════════════════════════════
// STConverter — construction
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn converter_construction_all_types() {
    for ct in &[
        ConvertType::T2S,
        ConvertType::S2T,
        ConvertType::TW2CN,
        ConvertType::CN2TW,
        ConvertType::T2HK,
        ConvertType::HK2T,
        ConvertType::JP2T,
    ] {
        let _c = STConverter::new(*ct);
    }
}

#[test]
fn converter_clone_copy() {
    let c = STConverter::new(ConvertType::T2S);
    let _c2 = c; // Copy
}

// ═══════════════════════════════════════════════════════════════════════════════
// STConverter — T2S (Traditional → Simplified)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t2s_basic() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("計算機科學與技術"), "计算机科学与技术");
}

#[test]
fn t2s_single_char() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("國"), "国");
}

#[test]
fn t2s_multi_char_mapping() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("鼠標"), "鼠标");
}

#[test]
fn t2s_mixed_with_ascii() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("Hello 憂鬱 World"), "Hello 忧郁 World");
}

// ═══════════════════════════════════════════════════════════════════════════════
// STConverter — S2T (Simplified → Traditional)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn s2t_basic() {
    let c = STConverter::new(ConvertType::S2T);
    assert_eq!(c.convert("计算机科学与技术"), "計算機科學與技術");
}

#[test]
fn s2t_single_char() {
    let c = STConverter::new(ConvertType::S2T);
    assert_eq!(c.convert("国"), "國");
}

#[test]
fn s2t_multi_char_mapping() {
    let c = STConverter::new(ConvertType::S2T);
    assert_eq!(c.convert("方便面"), "方便麪");
}

// ═══════════════════════════════════════════════════════════════════════════════
// STConverter — regional variants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn tw2cn() {
    let c = STConverter::new(ConvertType::TW2CN);
    assert_eq!(c.convert("记忆体"), "内存");
}

#[test]
fn cn2tw() {
    let c = STConverter::new(ConvertType::CN2TW);
    assert_eq!(c.convert("内存"), "记忆体");
}

#[test]
fn t2hk() {
    let c = STConverter::new(ConvertType::T2HK);
    assert_eq!(c.convert("衛"), "衞");
}

#[test]
fn hk2t() {
    let c = STConverter::new(ConvertType::HK2T);
    assert_eq!(c.convert("衞"), "衛");
}

#[test]
fn jp2t() {
    let c = STConverter::new(ConvertType::JP2T);
    assert_eq!(c.convert("両"), "兩");
}

// ═══════════════════════════════════════════════════════════════════════════════
// STConverter — edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn convert_empty_string() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert(""), "");
}

#[test]
fn convert_pure_ascii() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("hello world 123"), "hello world 123");
}

#[test]
fn convert_pure_digits() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("12345"), "12345");
}

#[test]
fn convert_single_ascii_char() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("a"), "a");
}

#[test]
fn convert_whitespace() {
    let c = STConverter::new(ConvertType::T2S);
    assert_eq!(c.convert("   "), "   ");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Free function API — convert() and convert_to()
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn free_fn_convert_t2s() {
    assert_eq!(convert("計算機", ConvertType::T2S), "计算机");
}

#[test]
fn free_fn_convert_s2t() {
    assert_eq!(convert("计算机", ConvertType::S2T), "計算機");
}

#[test]
fn free_fn_convert_to_buffer_reuse() {
    let mut buf = String::new();
    convert_to("傳統", ConvertType::T2S, &mut buf);
    assert_eq!(buf, "传统");
    buf.clear();
    convert_to("簡體", ConvertType::T2S, &mut buf);
    assert_eq!(buf, "简体");
}

// ═══════════════════════════════════════════════════════════════════════════════
// STConverter — convert_cow
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn convert_cow_no_change_borrows() {
    let c = STConverter::new(ConvertType::T2S);
    let result = c.convert_cow("hello ascii");
    assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn convert_cow_with_change_owns() {
    let c = STConverter::new(ConvertType::T2S);
    let result = c.convert_cow("傳統");
    assert!(matches!(result, std::borrow::Cow::Owned(_)));
    assert_eq!(result.as_ref(), "传统");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ConvertConfig
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn config_default() {
    let cfg = ConvertConfig::default();
    assert_eq!(cfg.convert_type, ConvertType::S2T);
    assert!(!cfg.keep_both);
    assert_eq!(cfg.delimiter, ",");
}

#[test]
fn config_builder() {
    let cfg = ConvertConfig::new(ConvertType::T2S)
        .with_keep_both(true)
        .with_delimiter("|");
    assert_eq!(cfg.convert_type, ConvertType::T2S);
    assert!(cfg.keep_both);
    assert_eq!(cfg.delimiter, "|");
}

#[test]
fn config_validate_ok() {
    let cfg = ConvertConfig::new(ConvertType::T2S)
        .with_keep_both(true)
        .with_delimiter(",");
    assert!(cfg.validate().is_ok());
}

#[test]
fn config_validate_empty_delimiter_fails() {
    let cfg = ConvertConfig::new(ConvertType::T2S)
        .with_keep_both(true)
        .with_delimiter("");
    assert!(cfg.validate().is_err());
}

#[test]
fn config_validate_no_keep_both_any_delimiter() {
    let cfg = ConvertConfig::new(ConvertType::T2S)
        .with_keep_both(false)
        .with_delimiter("");
    // When keep_both is false, delimiter doesn't matter
    assert!(cfg.validate().is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Unicode handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn convert_emoji_passthrough() {
    let c = STConverter::new(ConvertType::T2S);
    let result = c.convert("😀你好😊");
    assert!(result.contains("😀"));
    assert!(result.contains("😊"));
}

#[test]
fn convert_japanese_hiragana_passthrough() {
    let c = STConverter::new(ConvertType::T2S);
    let result = c.convert("こんにちは");
    assert_eq!(result, "こんにちは");
}

#[test]
fn convert_korean_passthrough() {
    let c = STConverter::new(ConvertType::T2S);
    let result = c.convert("안녕하세요");
    assert_eq!(result, "안녕하세요");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Roundtrip tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_t2s_s2t() {
    let t2s = STConverter::new(ConvertType::T2S);
    let s2t = STConverter::new(ConvertType::S2T);
    let original = "計算機科學與技術";
    let simplified = t2s.convert(original);
    let back = s2t.convert(&simplified);
    assert_eq!(back, original);
}
