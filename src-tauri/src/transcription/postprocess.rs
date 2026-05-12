use regex::Regex;

/// Post-process transcribed text: remove filler words, apply vocabulary
/// replacements, clean up whitespace
pub fn postprocess(
    text: &str,
    filler_words: &[String],
    remove_fillers: bool,
    vocabulary_replacements: &[(String, String)],
) -> String {
    let mut result = text.to_string();

    if remove_fillers && !filler_words.is_empty() {
        result = remove_filler_words(&result, filler_words);
    }

    // Apply vocabulary replacements (wrong -> correct)
    if !vocabulary_replacements.is_empty() {
        result = apply_vocabulary_replacements(&result, vocabulary_replacements);
    }

    // Clean up whitespace
    result = normalize_whitespace(&result);

    // Capitalize first letter
    result = capitalize_first(&result);

    result
}

/// Remove standalone filler words (case-insensitive, word-boundary aware)
fn remove_filler_words(text: &str, fillers: &[String]) -> String {
    let mut result = text.to_string();

    for filler in fillers {
        // Match the filler word at word boundaries, optionally followed by a comma
        let pattern = format!(r"(?i)\b{}\b,?\s*", regex::escape(filler));
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, " ").to_string();
        }
    }

    result
}

/// Normalize multiple spaces and trim
fn normalize_whitespace(text: &str) -> String {
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(text.trim(), " ").to_string()
}

/// Apply vocabulary word replacements (case-insensitive, whole-word).
/// Preserves the case pattern of the matched word in the text.
fn apply_vocabulary_replacements(text: &str, replacements: &[(String, String)]) -> String {
    let mut result = text.to_string();

    for (wrong, correct) in replacements {
        let pattern = format!(r"(?i)\b{}\b", regex::escape(wrong));
        if let Ok(re) = Regex::new(&pattern) {
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let matched = &caps[0];
                    apply_case_pattern(matched, correct)
                })
                .to_string();
        }
    }

    result
}

/// Apply the case pattern of `source` to `target`.
/// - If source is all uppercase, return target in all uppercase
/// - If source starts with uppercase, capitalize target
/// - Otherwise return target as-is (the learned correct form)
fn apply_case_pattern(source: &str, target: &str) -> String {
    if source
        .chars()
        .all(|c| !c.is_alphabetic() || c.is_uppercase())
    {
        target.to_uppercase()
    } else if source
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
        let mut chars = target.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    } else {
        target.to_string()
    }
}

/// Capitalize the first character of the string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filler_removal() {
        let fillers = vec!["um".to_string(), "uh".to_string()];
        let result = postprocess("um I think uh that this is good", &fillers, true, &[]);
        assert_eq!(result, "I think that this is good");
    }

    #[test]
    fn test_capitalization() {
        let result = postprocess("hello world", &[], false, &[]);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_whitespace_normalization() {
        let result = postprocess("  hello   world  ", &[], false, &[]);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_vocabulary_replacement_basic() {
        let replacements = vec![("Marshal".to_string(), "Marcel".to_string())];
        let result = postprocess("Hello Marshal, nice to meet you", &[], false, &replacements);
        assert_eq!(result, "Hello Marcel, nice to meet you");
    }

    #[test]
    fn test_vocabulary_replacement_case_insensitive() {
        let replacements = vec![("marshal".to_string(), "Marcel".to_string())];
        let result = postprocess("hello MARSHAL and marshal", &[], false, &replacements);
        assert_eq!(result, "Hello MARCEL and Marcel");
    }

    #[test]
    fn test_vocabulary_replacement_preserves_case() {
        let replacements = vec![("cubernetes".to_string(), "Kubernetes".to_string())];
        let result = postprocess("I use cubernetes daily", &[], false, &replacements);
        assert_eq!(result, "I use Kubernetes daily");
    }

    #[test]
    fn test_vocabulary_replacement_whole_word_only() {
        let replacements = vec![("art".to_string(), "ART".to_string())];
        let result = postprocess("the artist made art", &[], false, &replacements);
        // "artist" should NOT be affected, only standalone "art"
        assert_eq!(result, "The artist made ART");
    }

    #[test]
    fn test_vocabulary_replacement_multiple() {
        let replacements = vec![
            ("Marshal".to_string(), "Marcel".to_string()),
            ("cubernetes".to_string(), "Kubernetes".to_string()),
        ];
        let result = postprocess("Marshal uses cubernetes", &[], false, &replacements);
        assert_eq!(result, "Marcel uses Kubernetes");
    }

    #[test]
    fn test_vocabulary_with_fillers() {
        let fillers = vec!["um".to_string()];
        let replacements = vec![("Marshal".to_string(), "Marcel".to_string())];
        let result = postprocess("um my name is Marshal", &fillers, true, &replacements);
        assert_eq!(result, "My name is Marcel");
    }

    #[test]
    fn test_apply_case_pattern() {
        assert_eq!(apply_case_pattern("HELLO", "world"), "WORLD");
        assert_eq!(apply_case_pattern("Hello", "world"), "World");
        assert_eq!(apply_case_pattern("hello", "World"), "World");
    }
}
