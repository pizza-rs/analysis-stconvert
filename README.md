<div align="center">

# 🔀 pizza-analysis-stconvert

**Simplified/Traditional Chinese conversion plugin for [INFINI Pizza](https://pizza.rs)**

[![Crate](https://img.shields.io/badge/crate-pizza--analysis--stconvert-blue)](https://github.com/pizza-rs/analysis-stconvert)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

</div>

---

## Overview

`pizza-analysis-stconvert` provides bidirectional Simplified ↔ Traditional Chinese conversion for the [INFINI Pizza](https://pizza.rs) search engine, supporting multiple regional variants.

### Key Features

- **Bidirectional Conversion** — Simplified → Traditional and Traditional → Simplified
- **Regional Variants** — CN (Mainland), TW (Taiwan), HK (Hong Kong), JP (Japanese Kanji)
- **Dictionary-Based** — Accurate phrase-level conversion (not just character mapping)
- **Multiple Integration Points** — Available as tokenizer, filter, and normalizer

## Components

| Type | Name | Description |
|:-----|:-----|:------------|
| Tokenizer | `stconvert_s2t` | Simplified → Traditional tokenizer |
| Tokenizer | `stconvert_t2s` | Traditional → Simplified tokenizer |
| Filter | `stconvert_s2t` | Per-token S→T conversion |
| Filter | `stconvert_t2s` | Per-token T→S conversion |
| Normalizer | `stconvert_s2t` | Full-text S→T conversion |
| Normalizer | `stconvert_t2s` | Full-text T→S conversion |
| Analyzer | `stconvert_s2t` | S→T conversion analyzer |
| Analyzer | `stconvert_t2s` | T→S conversion analyzer |

## Example

```text
S→T: "中华人民共和国" → "中華人民共和國"
T→S: "計算機科學" → "计算机科学"
```

## Installation

```toml
[dependencies]
pizza-analysis-stconvert = "0.1"
```

Or via `pizza-analysis-all`:

```toml
[dependencies]
pizza-analysis-all = { version = "0.1", features = ["stconvert"] }
```

## Usage

```rust
use pizza_engine::analysis::AnalysisFactory;

let mut factory = AnalysisFactory::new();
pizza_analysis_stconvert::register_all(&mut factory);
```

## License

MIT

---

<div align="center">
<sub>Part of the <a href="https://pizza.rs">INFINI Pizza</a> ecosystem</sub>
</div>
