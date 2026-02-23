pub fn system_prompt(target_lang: &str, style: &str) -> String {
    let role = match style {
        "manga" => {
            "You are an expert manga/comics translator. You understand Japanese comic conventions, \
             onomatopoeia (擬音語/擬態語), speech bubble layouts, and reading order (right-to-left for Japanese manga). \
             Preserve the expressive tone of dialogue and translate sound effects contextually."
        }
        "novel" => {
            "You are an expert visual novel translator skilled in narrative prose, dialogue nuance, \
             honorifics, and literary register. Maintain the emotional tone, character voice distinctions, \
             and narrative flow of the original text."
        }
        "game" => {
            "You are an expert game translator specializing in UI text, menu items, quest descriptions, \
             skill names, and in-game dialogue. Keep translations concise to fit UI constraints. \
             Preserve proper nouns and game-specific terminology."
        }
        _ => {
            "You are an expert OCR and translation specialist. You accurately read text from images \
             and produce natural, fluent translations that preserve the original meaning and tone."
        }
    };

    format!(
        "{role}\n\n\
         TASK: Perform OCR on the provided image and translate the detected text into {target_lang}.\n\n\
         GUIDELINES:\n\
         - Read ALL visible text in the image, following natural reading order.\n\
         - If multiple text regions exist (speech bubbles, labels, signs), combine them with line breaks in reading order.\n\
         - Translate idioms and cultural expressions naturally rather than literally.\n\
         - Preserve the register and tone (formal/casual/comedic) of the original.\n\
         - For ambiguous text, use the visual context (character expressions, scene setting) to choose the best interpretation.\n\
         - Do NOT add explanations, notes, or commentary — only the OCR result and translation.\n\n\
         OUTPUT FORMAT: Respond with ONLY a JSON object, no markdown, no code fences, no surrounding text:\n\
         {{\"source_text\": \"<all detected text>\", \"translated_text\": \"<translation in {target_lang}>\"}}\n\n\
         If no text is found in the image:\n\
         {{\"source_text\": \"\", \"translated_text\": \"\"}}"
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct TranslationResponse {
    #[serde(default)]
    pub source_text: String,
    #[serde(default)]
    pub translated_text: String,
}

/// Extract a JSON object from potentially messy LLM output.
/// Handles markdown code fences, leading/trailing garbage, and truncated JSON.
pub fn extract_json(raw: &str) -> Result<TranslationResponse, String> {
    let trimmed = raw.trim();

    // Strip markdown code fences: ```json ... ``` or ``` ... ```
    let stripped = if trimmed.starts_with("```") {
        let inner = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```");
        inner.trim_end_matches("```").trim()
    } else {
        trimmed
    };

    // Try parsing directly first
    if let Ok(resp) = serde_json::from_str::<TranslationResponse>(stripped) {
        return Ok(resp);
    }

    // Try to find the JSON object by locating { ... }
    if let Some(start) = stripped.find('{') {
        let candidate = &stripped[start..];
        // Find matching closing brace
        if let Some(end) = candidate.rfind('}') {
            let json_str = &candidate[..=end];
            if let Ok(resp) = serde_json::from_str::<TranslationResponse>(json_str) {
                return Ok(resp);
            }
        }

        // JSON is truncated (EOF while parsing) — try to salvage partial content
        if let Some(resp) = salvage_truncated_json(candidate) {
            return Ok(resp);
        }
    }

    Err(format!(
        "Could not extract valid JSON from LLM response: {stripped}"
    ))
}

/// Attempt to extract fields from truncated/malformed JSON.
fn salvage_truncated_json(raw: &str) -> Option<TranslationResponse> {
    let mut source_text = String::new();
    let mut translated_text = String::new();

    // Try to extract "source_text": "..." using a simple pattern
    if let Some(val) = extract_json_string_field(raw, "source_text") {
        source_text = val;
    }
    if let Some(val) = extract_json_string_field(raw, "translated_text") {
        translated_text = val;
    }

    // Only return if we got at least something useful
    if !source_text.is_empty() || !translated_text.is_empty() {
        Some(TranslationResponse {
            source_text,
            translated_text,
        })
    } else {
        None
    }
}

/// Extract a string value for a given key from a JSON-like string.
/// Handles the case where the JSON is truncated mid-string.
fn extract_json_string_field(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let key_pos = raw.find(&pattern)?;
    let after_key = &raw[key_pos + pattern.len()..];
    // Skip optional whitespace and colon
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();
    // Expect opening quote
    let after_quote = after_colon.strip_prefix('"')?;
    // Find the closing quote (handling escaped quotes)
    let mut result = String::new();
    let mut chars = after_quote.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        '"' => result.push('"'),
                        '\\' => result.push('\\'),
                        _ => {
                            result.push('\\');
                            result.push(escaped);
                        }
                    }
                }
            }
            '"' => return Some(result),
            _ => result.push(ch),
        }
    }
    // Reached end without closing quote — return what we have (truncated)
    if !result.is_empty() {
        Some(result)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_contains_target_lang() {
        let prompt = system_prompt("简体中文", "general");
        assert!(prompt.contains("简体中文"));
        assert!(prompt.contains("JSON"));
        assert!(prompt.contains("source_text"));
        assert!(prompt.contains("translated_text"));
    }

    #[test]
    fn system_prompt_different_lang() {
        let prompt = system_prompt("English", "general");
        assert!(prompt.contains("English"));
        assert!(!prompt.contains("简体中文"));
    }

    #[test]
    fn parse_translation_response() {
        let json = r#"{"source_text": "こんにちは", "translated_text": "你好"}"#;
        let resp = extract_json(json).unwrap();
        assert_eq!(resp.source_text, "こんにちは");
        assert_eq!(resp.translated_text, "你好");
    }

    #[test]
    fn parse_empty_translation_response() {
        let json = r#"{"source_text": "", "translated_text": ""}"#;
        let resp = extract_json(json).unwrap();
        assert!(resp.source_text.is_empty());
        assert!(resp.translated_text.is_empty());
    }

    #[test]
    fn parse_markdown_fenced_json() {
        let raw = "```json\n{\"source_text\": \"hello\", \"translated_text\": \"你好\"}\n```";
        let resp = extract_json(raw).unwrap();
        assert_eq!(resp.source_text, "hello");
        assert_eq!(resp.translated_text, "你好");
    }

    #[test]
    fn parse_json_with_leading_text() {
        let raw =
            "Here is the translation:\n{\"source_text\": \"hi\", \"translated_text\": \"嗨\"}";
        let resp = extract_json(raw).unwrap();
        assert_eq!(resp.translated_text, "嗨");
    }

    #[test]
    fn parse_truncated_json() {
        // Simulates "EOF while parsing a string" — JSON cut off mid-value
        let raw = r#"{"source_text": "hello world", "translated_text": "你好世界"#;
        let resp = extract_json(raw).unwrap();
        assert_eq!(resp.source_text, "hello world");
        assert_eq!(resp.translated_text, "你好世界");
    }

    #[test]
    fn parse_truncated_json_source_only() {
        let raw = r#"{"source_text": "some text"#;
        let resp = extract_json(raw).unwrap();
        assert_eq!(resp.source_text, "some text");
        assert!(resp.translated_text.is_empty());
    }

    #[test]
    fn manga_style_prompt() {
        let prompt = system_prompt("简体中文", "manga");
        assert!(prompt.contains("manga/comics translator"));
    }

    #[test]
    fn novel_style_prompt() {
        let prompt = system_prompt("English", "novel");
        assert!(prompt.contains("visual novel translator"));
    }

    #[test]
    fn game_style_prompt() {
        let prompt = system_prompt("English", "game");
        assert!(prompt.contains("game translator"));
    }
}
