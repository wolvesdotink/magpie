//! User-defined text-transform pipeline.
//!
//! Compiles `TextTransform` values into regex / literal matchers once at
//! resolution time so the postprocess hot path doesn't re-compile per word.

use regex::{Regex, RegexBuilder};

use crate::styles::{TextTransform, TransformKind};

/// A `TextTransform` whose regex patterns (if any) have been compiled and
/// validated. Produced by `compile_all` from a `&[TextTransform]`.
#[derive(Debug)]
pub enum CompiledTransform {
    Replace { regex: Regex, replacement: String },
    Prepend { text: String },
    Append { text: String },
    TrimEdges,
    SqueezeChars { regex: Regex },
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("transform {index} (\"{label}\") has invalid regex: {message}")]
    InvalidRegex {
        index: usize,
        label: String,
        message: String,
    },
    #[error("transform {index} (\"{label}\") has invalid pattern: {message}")]
    InvalidPattern {
        index: usize,
        label: String,
        message: String,
    },
}

/// Compile every enabled transform in `transforms`. Disabled entries are
/// skipped. The returned vec preserves ordering.
pub fn compile_all(transforms: &[TextTransform]) -> Result<Vec<CompiledTransform>, CompileError> {
    let mut out = Vec::new();
    for (idx, t) in transforms.iter().enumerate() {
        if !t.enabled {
            continue;
        }
        let label = t
            .label
            .clone()
            .unwrap_or_else(|| format!("rule {}", idx + 1));
        match compile_one(&t.kind, idx, &label) {
            Ok(compiled) => out.push(compiled),
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

fn compile_one(
    kind: &TransformKind,
    index: usize,
    label: &str,
) -> Result<CompiledTransform, CompileError> {
    match kind {
        TransformKind::Replace {
            pattern,
            replacement,
            is_regex,
            case_sensitive,
            whole_word,
        } => {
            if pattern.is_empty() {
                return Err(CompileError::InvalidPattern {
                    index,
                    label: label.to_string(),
                    message: "pattern cannot be empty".into(),
                });
            }
            let pattern_str = if *is_regex {
                pattern.clone()
            } else {
                let escaped = regex::escape(pattern);
                if *whole_word {
                    format!(r"\b{}\b", escaped)
                } else {
                    escaped
                }
            };
            let regex = RegexBuilder::new(&pattern_str)
                .case_insensitive(!case_sensitive)
                .build()
                .map_err(|e| CompileError::InvalidRegex {
                    index,
                    label: label.to_string(),
                    message: e.to_string(),
                })?;
            Ok(CompiledTransform::Replace {
                regex,
                replacement: replacement.clone(),
            })
        }
        TransformKind::Prepend { text } => Ok(CompiledTransform::Prepend { text: text.clone() }),
        TransformKind::Append { text } => Ok(CompiledTransform::Append { text: text.clone() }),
        TransformKind::TrimEdges => Ok(CompiledTransform::TrimEdges),
        TransformKind::SqueezeChars { chars } => {
            if chars.is_empty() {
                return Err(CompileError::InvalidPattern {
                    index,
                    label: label.to_string(),
                    message: "chars cannot be empty".into(),
                });
            }
            // Build a character class regex from the literal chars.
            let mut class = String::from("[");
            for c in chars.chars() {
                // Escape regex specials inside the class
                if matches!(c, '\\' | ']' | '[' | '^' | '-') {
                    class.push('\\');
                }
                class.push(c);
            }
            class.push_str("]+");
            let regex = Regex::new(&class).map_err(|e| CompileError::InvalidRegex {
                index,
                label: label.to_string(),
                message: e.to_string(),
            })?;
            Ok(CompiledTransform::SqueezeChars { regex })
        }
    }
}

/// Apply the pipeline to a piece of text, in order.
pub fn apply(text: &str, transforms: &[CompiledTransform]) -> String {
    let mut current = text.to_string();
    for t in transforms {
        current = match t {
            CompiledTransform::Replace { regex, replacement } => regex
                .replace_all(&current, replacement.as_str())
                .into_owned(),
            CompiledTransform::Prepend { text } => format!("{}{}", text, current),
            CompiledTransform::Append { text } => format!("{}{}", current, text),
            CompiledTransform::TrimEdges => current.trim().to_string(),
            CompiledTransform::SqueezeChars { regex } => {
                regex.replace_all(&current, " ").into_owned()
            }
        };
    }
    current
}

/// Validate a single transform — used by the UI to surface regex errors as
/// the user types.
pub fn validate(transform: &TextTransform) -> Result<(), String> {
    compile_one(
        &transform.kind,
        0,
        transform.label.as_deref().unwrap_or("rule"),
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(enabled: bool, kind: TransformKind) -> TextTransform {
        TextTransform {
            id: "test".into(),
            enabled,
            label: None,
            kind,
        }
    }

    #[test]
    fn literal_replace_works() {
        let transforms = vec![t(
            true,
            TransformKind::Replace {
                pattern: "world".into(),
                replacement: "earth".into(),
                is_regex: false,
                case_sensitive: false,
                whole_word: false,
            },
        )];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("hello world", &compiled), "hello earth");
    }

    #[test]
    fn case_insensitive_by_default() {
        let transforms = vec![t(
            true,
            TransformKind::Replace {
                pattern: "WORLD".into(),
                replacement: "earth".into(),
                is_regex: false,
                case_sensitive: false,
                whole_word: false,
            },
        )];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("hello world", &compiled), "hello earth");
    }

    #[test]
    fn case_sensitive_when_requested() {
        let transforms = vec![t(
            true,
            TransformKind::Replace {
                pattern: "WORLD".into(),
                replacement: "earth".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: false,
            },
        )];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("hello world", &compiled), "hello world");
    }

    #[test]
    fn whole_word_avoids_substring_match() {
        let transforms = vec![t(
            true,
            TransformKind::Replace {
                pattern: "art".into(),
                replacement: "ART".into(),
                is_regex: false,
                case_sensitive: false,
                whole_word: true,
            },
        )];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(
            apply("the artist made art", &compiled),
            "the artist made ART"
        );
    }

    #[test]
    fn regex_replace_works() {
        let transforms = vec![t(
            true,
            TransformKind::Replace {
                pattern: r"\bhello\b".into(),
                replacement: "hi".into(),
                is_regex: true,
                case_sensitive: false,
                whole_word: false,
            },
        )];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("hello world", &compiled), "hi world");
    }

    #[test]
    fn invalid_regex_errors() {
        let transforms = vec![t(
            true,
            TransformKind::Replace {
                pattern: "(unclosed".into(),
                replacement: "x".into(),
                is_regex: true,
                case_sensitive: false,
                whole_word: false,
            },
        )];
        let err = compile_all(&transforms).unwrap_err();
        assert!(matches!(err, CompileError::InvalidRegex { .. }));
    }

    #[test]
    fn disabled_transforms_are_skipped() {
        let transforms = vec![t(
            false,
            TransformKind::Replace {
                pattern: "(unclosed".into(),
                replacement: "x".into(),
                is_regex: true,
                case_sensitive: false,
                whole_word: false,
            },
        )];
        let compiled = compile_all(&transforms).unwrap();
        assert!(compiled.is_empty());
        assert_eq!(apply("hello", &compiled), "hello");
    }

    #[test]
    fn prepend_works() {
        let transforms = vec![t(true, TransformKind::Prepend { text: "[ ".into() })];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("note", &compiled), "[ note");
    }

    #[test]
    fn append_works() {
        let transforms = vec![t(true, TransformKind::Append { text: " ]".into() })];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("note", &compiled), "note ]");
    }

    #[test]
    fn trim_edges_works() {
        let transforms = vec![t(true, TransformKind::TrimEdges)];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("  hi  ", &compiled), "hi");
    }

    #[test]
    fn squeeze_chars_works() {
        let transforms = vec![t(true, TransformKind::SqueezeChars { chars: ".,".into() })];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("a,,,b...c", &compiled), "a b c");
    }

    #[test]
    fn pipeline_order_preserved() {
        let transforms = vec![
            t(
                true,
                TransformKind::Replace {
                    pattern: " ".into(),
                    replacement: "_".into(),
                    is_regex: false,
                    case_sensitive: false,
                    whole_word: false,
                },
            ),
            t(true, TransformKind::Prepend { text: "#".into() }),
        ];
        let compiled = compile_all(&transforms).unwrap();
        assert_eq!(apply("hello world", &compiled), "#hello_world");
    }

    #[test]
    fn validate_rejects_empty_pattern() {
        let bad = t(
            true,
            TransformKind::Replace {
                pattern: "".into(),
                replacement: "x".into(),
                is_regex: false,
                case_sensitive: false,
                whole_word: false,
            },
        );
        assert!(validate(&bad).is_err());
    }

    #[test]
    fn validate_accepts_good_regex() {
        let good = t(
            true,
            TransformKind::Replace {
                pattern: r"\d+".into(),
                replacement: "N".into(),
                is_regex: true,
                case_sensitive: true,
                whole_word: false,
            },
        );
        assert!(validate(&good).is_ok());
    }
}
