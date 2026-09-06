//! Register ST Convert (Simplified/Traditional Chinese) components into [`AnalysisFactory`].

use alloc::boxed::Box;
use alloc::vec;

use pizza_engine::analysis::AnalysisFactory;
use pizza_engine::analysis::Analyzer;

use crate::ConvertConfig;
use crate::ConvertType;
use crate::STConvertNormalizer;
use crate::STConvertTokenFilter;
use crate::STConvertTokenizer;

/// Register ST Convert normalizers, tokenizers, token filters, and analyzers.
///
/// Matches Elasticsearch's analysis-stconvert plugin registration:
/// - Java registers a single `stconvert` name for tokenizer, filter, char_filter, and analyzer
///   with the convert type configured via settings.
/// - In our Rust implementation, we pre-register directional variants (`_s2t`, `_t2s`)
///   since we use pre-configured instances rather than factory-based settings.
pub fn register_all(factory: &mut AnalysisFactory) {
    // Normalizers (pre-tokenization, equivalent to char_filter in ES)
    factory.register_normalizer_with("stconvert_s2t", || {
        Box::new(STConvertNormalizer::new(ConvertConfig::new(
            ConvertType::S2T,
        )))
    });
    factory.register_normalizer_with("stconvert_t2s", || {
        Box::new(STConvertNormalizer::new(ConvertConfig::new(
            ConvertType::T2S,
        )))
    });

    // Token filters (post-tokenization)
    factory.register_token_filter_with("stconvert_s2t", || {
        Box::new(STConvertTokenFilter::new(ConvertConfig::new(
            ConvertType::S2T,
        )))
    });
    factory.register_token_filter_with("stconvert_t2s", || {
        Box::new(STConvertTokenFilter::new(ConvertConfig::new(
            ConvertType::T2S,
        )))
    });

    // Tokenizers
    factory.register_tokenizer_with("stconvert_s2t", || {
        Box::new(STConvertTokenizer::new(ConvertConfig::new(
            ConvertType::S2T,
        )))
    });
    factory.register_tokenizer_with("stconvert_t2s", || {
        Box::new(STConvertTokenizer::new(ConvertConfig::new(
            ConvertType::T2S,
        )))
    });

    // Analyzers (just tokenizer, matches Java STConvertAnalyzer)
    factory.register_analyzer_with("stconvert_s2t", || {
        Analyzer::new(
            vec![],
            Box::new(STConvertTokenizer::new(ConvertConfig::new(
                ConvertType::S2T,
            ))),
            vec![],
        )
    });
    factory.register_analyzer_with("stconvert_t2s", || {
        Analyzer::new(
            vec![],
            Box::new(STConvertTokenizer::new(ConvertConfig::new(
                ConvertType::T2S,
            ))),
            vec![],
        )
    });
}
