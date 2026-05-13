//! Casing transforms for the postprocess pipeline.

use crate::styles::CasingMode;

/// Apply the chosen casing mode to `text`.
pub fn apply(text: &str, mode: CasingMode, auto_capitalize_after_sentence: bool) -> String {
    if text.is_empty() {
        return String::new();
    }
    match mode {
        CasingMode::Sentence => sentence(text, auto_capitalize_after_sentence),
        CasingMode::Preserve => text.to_string(),
        CasingMode::Lowercase => text.to_lowercase(),
        CasingMode::Uppercase => text.to_uppercase(),
        CasingMode::SnakeCase => identifier_join(text, '_', false),
        CasingMode::KebabCase => identifier_join(text, '-', false),
        CasingMode::CamelCase => camel_or_pascal(text, false),
        CasingMode::PascalCase => camel_or_pascal(text, true),
        CasingMode::ScreamSnake => identifier_join(text, '_', true),
    }
}

fn sentence(text: &str, after_sentence: bool) -> String {
    // Capitalize the first ALPHABETIC character (skipping leading whitespace
    // and punctuation). Otherwise leading whitespace would defeat the upper-
    // casing — which surprised users in the wild.
    let mut out = String::with_capacity(text.len());
    let mut first_done = false;
    let mut capitalize_next = false;
    for c in text.chars() {
        if !first_done && c.is_alphabetic() {
            for u in c.to_uppercase() {
                out.push(u);
            }
            first_done = true;
            capitalize_next = false;
            continue;
        }
        if after_sentence && capitalize_next && c.is_alphabetic() {
            for u in c.to_uppercase() {
                out.push(u);
            }
            capitalize_next = false;
            continue;
        }
        out.push(c);
        if after_sentence {
            if matches!(c, '.' | '!' | '?') {
                capitalize_next = true;
            } else if !c.is_whitespace() && !matches!(c, '"' | '\'' | ')' | ']' | '}') {
                capitalize_next = false;
            }
        }
    }
    out
}

fn tokenize_for_identifier(text: &str) -> Vec<String> {
    // Split on whitespace AND on punctuation, then drop empty tokens.
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn identifier_join(text: &str, sep: char, upper: bool) -> String {
    let tokens = tokenize_for_identifier(text);
    let cased: Vec<String> = tokens
        .into_iter()
        .map(|t| {
            if upper {
                t.to_uppercase()
            } else {
                t.to_lowercase()
            }
        })
        .collect();
    cased.join(&sep.to_string())
}

fn camel_or_pascal(text: &str, pascal: bool) -> String {
    let tokens = tokenize_for_identifier(text);
    let mut out = String::new();
    for (idx, token) in tokens.into_iter().enumerate() {
        let lower = token.to_lowercase();
        if idx == 0 && !pascal {
            out.push_str(&lower);
        } else {
            let mut chars = lower.chars();
            if let Some(first) = chars.next() {
                for u in first.to_uppercase() {
                    out.push(u);
                }
                out.push_str(chars.as_str());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_default() {
        assert_eq!(
            apply("hello world", CasingMode::Sentence, false),
            "Hello world"
        );
    }

    #[test]
    fn sentence_with_auto_after() {
        assert_eq!(
            apply(
                "hello world. how are you? great! see ya.",
                CasingMode::Sentence,
                true
            ),
            "Hello world. How are you? Great! See ya."
        );
    }

    #[test]
    fn preserve_passes_through() {
        assert_eq!(
            apply("Hello World", CasingMode::Preserve, false),
            "Hello World"
        );
    }

    #[test]
    fn lowercase_works() {
        assert_eq!(
            apply("Hello World", CasingMode::Lowercase, false),
            "hello world"
        );
    }

    #[test]
    fn uppercase_works() {
        assert_eq!(
            apply("Hello World", CasingMode::Uppercase, false),
            "HELLO WORLD"
        );
    }

    #[test]
    fn snake_case_splits_on_whitespace_and_punctuation() {
        assert_eq!(
            apply("Create User Profile, please.", CasingMode::SnakeCase, false),
            "create_user_profile_please"
        );
    }

    #[test]
    fn kebab_case_works() {
        assert_eq!(
            apply("My Project Name", CasingMode::KebabCase, false),
            "my-project-name"
        );
    }

    #[test]
    fn camel_case_works() {
        assert_eq!(
            apply("create user profile", CasingMode::CamelCase, false),
            "createUserProfile"
        );
    }

    #[test]
    fn pascal_case_works() {
        assert_eq!(
            apply("create user profile", CasingMode::PascalCase, false),
            "CreateUserProfile"
        );
    }

    #[test]
    fn scream_snake_works() {
        assert_eq!(
            apply("max retry count", CasingMode::ScreamSnake, false),
            "MAX_RETRY_COUNT"
        );
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(apply("", CasingMode::SnakeCase, false), "");
        assert_eq!(apply("", CasingMode::Sentence, true), "");
    }

    #[test]
    fn snake_collapses_multiple_separators() {
        assert_eq!(
            apply("hello,  world!!  foo", CasingMode::SnakeCase, false),
            "hello_world_foo"
        );
    }
}
