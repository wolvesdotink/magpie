//! Cleanup shared by local correction and translation. Preserve meaningful
//! Unicode (including joiners used in scripts and emoji).
pub fn clean_response(text: &str) -> Option<String> {
    let mut answer = text.trim();
    while let Some(reasoning) = answer.strip_prefix("<think>") {
        let end = reasoning.find("</think>")?;
        answer = reasoning[end + "</think>".len()..].trim_start();
    }
    // Some thinking models return a closing tag without the opening tag,
    // because the latter was supplied in the assistant prefill.
    if let Some(end) = answer.find("</think>") {
        answer = answer[end + "</think>".len()..].trim_start();
    }
    if answer.contains("<think>") {
        return None;
    }
    let cleaned = answer.replace(['\u{200b}', '\u{feff}'], "");
    let cleaned = cleaned.trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_reasoning_and_empty_artifacts() {
        assert_eq!(
            clean_response(" <think>analysis</think>\nHello\u{200b}."),
            Some("Hello.".into())
        );
        assert_eq!(
            clean_response("analysis</think>Bonjour."),
            Some("Bonjour.".into())
        );
        assert_eq!(clean_response("<think>unfinished"), None);
        assert_eq!(clean_response("\u{feff}\u{200b}"), None);
    }

    #[test]
    fn preserves_script_and_emoji_joiners() {
        let text = "می\u{200c}روم 👩\u{200d}💻";
        assert_eq!(clean_response(text).as_deref(), Some(text));
    }
}
