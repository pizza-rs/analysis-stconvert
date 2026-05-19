//! Configuration for Simplified/Traditional Chinese conversion.
//!
//! Follows the zero-cost configuration pattern: all config is validated
//! at construction time and stored as `Copy`/`'static` types to avoid
//! allocation on the hot path.

/// Conversion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConvertType {
    /// Traditional Chinese → Simplified Chinese
    T2S,
    /// Simplified Chinese → Traditional Chinese
    S2T,
    /// Taiwan Traditional → Mainland Simplified
    TW2CN,
    /// Mainland Simplified → Taiwan Traditional
    CN2TW,
    /// Traditional → Hong Kong variant
    T2HK,
    /// Hong Kong → Traditional
    HK2T,
    /// Japanese Kanji ↔ Traditional Chinese (JIS variants → standard)
    JP2T,
}

/// Configuration for the ST converter.
///
/// Uses `&'static str` for the delimiter to avoid any runtime allocation.
/// Call [`ConvertConfig::validate()`] after construction to ensure settings
/// are consistent.
#[derive(Debug, Clone, Copy)]
pub struct ConvertConfig {
    /// Conversion direction.
    pub convert_type: ConvertType,
    /// When `true`, emit both the original and converted forms.
    /// Useful in search indexing scenarios.
    pub keep_both: bool,
    /// Delimiter inserted between original and converted forms when
    /// `keep_both` is `true`. Points to a `'static` string to avoid
    /// allocation.
    pub delimiter: &'static str,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            convert_type: ConvertType::S2T,
            keep_both: false,
            delimiter: ",",
        }
    }
}

impl ConvertConfig {
    /// Create a new config for the given direction with defaults.
    #[inline]
    pub fn new(convert_type: ConvertType) -> Self {
        Self {
            convert_type,
            ..Default::default()
        }
    }

    /// Builder: set `keep_both` mode.
    #[inline]
    pub fn with_keep_both(mut self, keep_both: bool) -> Self {
        self.keep_both = keep_both;
        self
    }

    /// Builder: set the delimiter (must be `'static`).
    #[inline]
    pub fn with_delimiter(mut self, delimiter: &'static str) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Validate that the configuration is consistent.
    /// Returns `Err` with a description if invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.keep_both && self.delimiter.is_empty() {
            return Err("delimiter must not be empty when keep_both is true");
        }
        Ok(())
    }
}
