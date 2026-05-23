#![cfg_attr(not(feature = "std"), no_std)]
//! # pizza-stconvert
//!
//! High-performance Simplified/Traditional Chinese converter with
//! zero-allocation fast path for single-character mappings.
//!
//! ## Quick start
//!
//! ```
//! use pizza_analysis_stconvert::{convert, ConvertType};
//!
//! // Traditional → Simplified
//! assert_eq!(convert("計算機科學與技術", ConvertType::T2S), "计算机科学与技术");
//!
//! // Simplified → Traditional
//! assert_eq!(convert("计算机科学与技术", ConvertType::S2T), "計算機科學與技術");
//! ```
//!
//! ## Buffer reuse for high throughput
//!
//! ```
//! use pizza_analysis_stconvert::{STConverter, ConvertType};
//!
//! let converter = STConverter::new(ConvertType::T2S);
//! let mut buf = String::new();
//!
//! for text in &["傳統", "簡體"] {
//!     buf.clear();
//!     converter.convert_to(text, &mut buf);
//!     // use buf...
//! }
//! ```
extern crate alloc;
mod config;
mod converter;
mod dict;

#[cfg(feature = "engine")]
mod tokenizer;

// Public API — keep flat and minimal.
pub use config::{ConvertConfig, ConvertType};
pub use converter::{convert, convert_to, STConverter};

#[cfg(feature = "engine")]
pub use tokenizer::{STConvertNormalizer, STConvertTokenFilter, STConvertTokenizer};
#[cfg(feature = "engine")]
pub mod register;
#[cfg(feature = "engine")]
pub use register::register_all;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t2s_basic() {
        let c = STConverter::new(ConvertType::T2S);
        assert_eq!(c.convert("計算機科學與技術"), "计算机科学与技术");
    }

    #[test]
    fn test_s2t_basic() {
        let c = STConverter::new(ConvertType::S2T);
        assert_eq!(c.convert("计算机科学与技术"), "計算機科學與技術");
    }

    #[test]
    fn test_t2s_multi_char() {
        let c = STConverter::new(ConvertType::T2S);
        // Multi-char keys in S2T dict (test T2S counterpart)
        assert_eq!(c.convert("鼠標"), "鼠标");
    }

    #[test]
    fn test_s2t_multi_char() {
        let c = STConverter::new(ConvertType::S2T);
        assert_eq!(c.convert("方便面"), "方便麪");
    }

    #[test]
    fn test_tw2cn() {
        let c = STConverter::new(ConvertType::TW2CN);
        assert_eq!(c.convert("记忆体"), "内存");
    }

    #[test]
    fn test_cn2tw() {
        let c = STConverter::new(ConvertType::CN2TW);
        assert_eq!(c.convert("内存"), "记忆体");
    }

    #[test]
    fn test_t2hk() {
        let c = STConverter::new(ConvertType::T2HK);
        assert_eq!(c.convert("衛"), "衞");
    }

    #[test]
    fn test_hk2t() {
        let c = STConverter::new(ConvertType::HK2T);
        assert_eq!(c.convert("衞"), "衛");
    }

    #[test]
    fn test_jp2t() {
        let c = STConverter::new(ConvertType::JP2T);
        assert_eq!(c.convert("両"), "兩");
    }

    #[test]
    fn test_passthrough_ascii() {
        let c = STConverter::new(ConvertType::T2S);
        assert_eq!(c.convert("hello world 123"), "hello world 123");
    }

    #[test]
    fn test_empty_input() {
        let c = STConverter::new(ConvertType::T2S);
        assert_eq!(c.convert(""), "");
    }

    #[test]
    fn test_mixed_content() {
        let c = STConverter::new(ConvertType::T2S);
        assert_eq!(c.convert("Hello 憂鬱 World"), "Hello 忧郁 World");
    }

    #[test]
    fn test_convert_cow_no_change() {
        let c = STConverter::new(ConvertType::T2S);
        let result = c.convert_cow("hello ascii");
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn test_convert_to_buffer_reuse() {
        let c = STConverter::new(ConvertType::T2S);
        let mut buf = String::new();
        c.convert_to("傳統", &mut buf);
        assert_eq!(buf, "传统");
        buf.clear();
        c.convert_to("簡體", &mut buf);
        assert_eq!(buf, "简体");
    }

    #[test]
    fn test_config_validation() {
        let cfg = ConvertConfig::new(ConvertType::T2S)
            .with_keep_both(true)
            .with_delimiter("");
        assert!(cfg.validate().is_err());

        let cfg = ConvertConfig::new(ConvertType::T2S)
            .with_keep_both(true)
            .with_delimiter(",");
        assert!(cfg.validate().is_ok());
    }
}
