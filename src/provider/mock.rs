use super::{ProviderError, TranslationResult};

pub struct MockProvider {
    response: Result<TranslationResult, ProviderError>,
}

impl MockProvider {
    pub fn new(source_text: &str, translated_text: &str) -> Self {
        Self {
            response: Ok(TranslationResult {
                source_text: source_text.to_string(),
                translated_text: translated_text.to_string(),
            }),
        }
    }

    pub fn with_error(error: ProviderError) -> Self {
        Self {
            response: Err(error),
        }
    }

    pub async fn translate(
        &self,
        _image_data: &[u8],
        _target_lang: &str,
        _style: &str,
    ) -> Result<TranslationResult, ProviderError> {
        match &self.response {
            Ok(result) => Ok(result.clone()),
            Err(e) => Err(match e {
                ProviderError::RequestFailed(msg) => ProviderError::RequestFailed(msg.clone()),
                ProviderError::ParseError(msg) => ProviderError::ParseError(msg.clone()),
                ProviderError::AuthError(msg) => ProviderError::AuthError(msg.clone()),
                ProviderError::Timeout(secs) => ProviderError::Timeout(*secs),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_provider_returns_translation() {
        let provider = MockProvider::new("こんにちは", "你好");
        let result = provider
            .translate(b"fake_image", "简体中文", "general")
            .await
            .unwrap();
        assert_eq!(result.source_text, "こんにちは");
        assert_eq!(result.translated_text, "你好");
    }

    #[tokio::test]
    async fn mock_provider_returns_error() {
        let provider = MockProvider::with_error(ProviderError::Timeout(15));
        let err = provider
            .translate(b"fake_image", "简体中文", "general")
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Timeout(15)));
    }

    #[tokio::test]
    async fn mock_provider_ignores_input() {
        let provider = MockProvider::new("test", "翻译");
        let r1 = provider
            .translate(b"image1", "简体中文", "manga")
            .await
            .unwrap();
        let r2 = provider
            .translate(b"different_image", "English", "novel")
            .await
            .unwrap();
        assert_eq!(r1.source_text, r2.source_text);
    }
}
