/// Detect the system locale and return an appropriate target language name.
pub fn detect_target_lang() -> String {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();

    locale_to_lang(&locale).to_string()
}

/// Map a locale string (e.g. "zh_CN.UTF-8") to a human-readable language name.
fn locale_to_lang(locale: &str) -> &str {
    // Extract the language_TERRITORY part before any encoding suffix
    let base = locale.split('.').next().unwrap_or("");

    match base {
        s if s.starts_with("zh_CN") => "简体中文",
        s if s.starts_with("zh_TW") || s.starts_with("zh_HK") => "繁體中文",
        s if s.starts_with("ja") => "日本語",
        s if s.starts_with("ko") => "한국어",
        s if s.starts_with("fr") => "Français",
        s if s.starts_with("de") => "Deutsch",
        s if s.starts_with("es") => "Español",
        s if s.starts_with("pt_BR") => "Português (Brasil)",
        s if s.starts_with("pt") => "Português",
        s if s.starts_with("ru") => "Русский",
        s if s.starts_with("it") => "Italiano",
        s if s.starts_with("vi") => "Tiếng Việt",
        s if s.starts_with("th") => "ไทย",
        s if s.starts_with("ar") => "العربية",
        s if s.starts_with("en") => "English",
        _ => "English",
    }
}

/// Known languages for interactive selection.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh_CN", "简体中文"),
    ("zh_TW", "繁體中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("pt_BR", "Português (Brasil)"),
    ("ru", "Русский"),
    ("it", "Italiano"),
    ("vi", "Tiếng Việt"),
    ("th", "ไทย"),
    ("ar", "العربية"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_zh_cn() {
        assert_eq!(locale_to_lang("zh_CN.UTF-8"), "简体中文");
    }

    #[test]
    fn locale_zh_tw() {
        assert_eq!(locale_to_lang("zh_TW.UTF-8"), "繁體中文");
    }

    #[test]
    fn locale_ja() {
        assert_eq!(locale_to_lang("ja_JP.UTF-8"), "日本語");
    }

    #[test]
    fn locale_en() {
        assert_eq!(locale_to_lang("en_US.UTF-8"), "English");
    }

    #[test]
    fn locale_empty_fallback() {
        assert_eq!(locale_to_lang(""), "English");
    }

    #[test]
    fn locale_unknown_fallback() {
        assert_eq!(locale_to_lang("xx_XX.UTF-8"), "English");
    }

    #[test]
    fn locale_no_encoding() {
        assert_eq!(locale_to_lang("ko_KR"), "한국어");
    }
}
