//! Tokenizer, TokenFilter, and Normalizer integration with `pizza-engine`.
//!
//! - [`STConvertNormalizer`]: converts the full input text in-place before
//!   tokenization (useful as a pre-tokenization normalization step).
//! - [`STConvertTokenizer`]: converts the entire input text and emits a
//!   single token (useful for whole-field conversion).
//! - [`STConvertTokenFilter`]: converts each token's term in-place (useful
//!   for per-token normalization in an analysis chain). Supports `keep_both`
//!   mode where both original and converted forms are emitted.

use alloc::borrow::Cow;

use pizza_engine::analysis::{
    Normalizer, NormalizerClone, Token, TokenFilter, TokenFilterClone, Tokenizer, TokenizerClone,
};

use crate::config::ConvertConfig;
use crate::converter::STConverter;

/// Normalizer that converts the full input text in-place (Simplified ↔ Traditional).
///
/// Use this as a pre-tokenization step in an analysis chain to normalize
/// Chinese variant forms before any tokenizer splits the text.
#[derive(Clone)]
pub struct STConvertNormalizer {
    converter: STConverter,
}

impl STConvertNormalizer {
    pub fn new(config: ConvertConfig) -> Self {
        Self {
            converter: STConverter::new(config.convert_type),
        }
    }
}

impl Normalizer for STConvertNormalizer {
    fn normalize(&self, text: &mut String) {
        let converted = self.converter.convert(text.as_str());
        if converted != *text {
            *text = converted;
        }
    }
}

/// Tokenizer that converts the full input text and emits it as a single token.
#[derive(Clone)]
pub struct STConvertTokenizer {
    config: ConvertConfig,
    converter: STConverter,
}

impl STConvertTokenizer {
    pub fn new(config: ConvertConfig) -> Self {
        Self {
            converter: STConverter::new(config.convert_type),
            config,
        }
    }
}

impl Tokenizer for STConvertTokenizer {
    fn tokenize<'a>(&self, input: &'a str) -> Vec<Token<'a>> {
        if input.is_empty() {
            return Vec::new();
        }
        let converted = self.converter.convert(input);
        let end = input.len() as u32;

        if self.config.keep_both && converted != input {
            let mut combined = String::with_capacity(
                input.len() + self.config.delimiter.len() + converted.len(),
            );
            combined.push_str(input);
            combined.push_str(self.config.delimiter);
            combined.push_str(&converted);
            vec![Token {
                term: Cow::Owned(combined),
                start_offset: 0,
                end_offset: end,
                position: 0,
            }]
        } else {
            vec![Token {
                term: Cow::Owned(converted),
                start_offset: 0,
                end_offset: end,
                position: 0,
            }]
        }
    }
}

/// Token filter that converts each token's term individually.
///
/// With `keep_both = true`, emits an additional token at the same position
/// containing the converted form.
#[derive(Clone)]
pub struct STConvertTokenFilter {
    config: ConvertConfig,
    converter: STConverter,
}

impl STConvertTokenFilter {
    pub fn new(config: ConvertConfig) -> Self {
        Self {
            converter: STConverter::new(config.convert_type),
            config,
        }
    }
}

impl TokenFilter for STConvertTokenFilter {
    fn filter<'a>(&self, token: &mut Token<'a>) -> (bool, Option<Vec<Token<'a>>>) {
        let original = token.term.to_string();
        let converted = self.converter.convert(&original);

        if converted == original {
            // No change needed.
            return (true, None);
        }

        if self.config.keep_both {
            // Keep the original, emit converted as extra token at same position.
            let extra = Token {
                term: Cow::Owned(converted),
                start_offset: token.start_offset,
                end_offset: token.end_offset,
                position: token.position,
            };
            (true, Some(vec![extra]))
        } else {
            // Replace the token's term with the converted form.
            token.term = Cow::Owned(converted);
            (true, None)
        }
    }
}


