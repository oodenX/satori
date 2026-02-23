pub mod cloud;
pub mod local;
#[cfg(test)]
pub mod mock;

use crate::config::Profile;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub source_text: String,
    pub translated_text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("API request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse API response: {0}")]
    ParseError(String),
    #[error("API key not available: {0}")]
    #[allow(dead_code)]
    AuthError(String),
    #[error("Request timed out after {0} seconds")]
    Timeout(u64),
}

/// Enum-based dispatch to avoid async trait dyn-compatibility issues.
pub enum AnyProvider {
    Cloud(cloud::CloudProvider),
    Local(local::LocalProvider),
    #[cfg(test)]
    Mock(mock::MockProvider),
}

impl AnyProvider {
    pub async fn translate(
        &self,
        image_data: &[u8],
        target_lang: &str,
        style: &str,
    ) -> Result<TranslationResult, ProviderError> {
        match self {
            Self::Cloud(p) => p.translate(image_data, target_lang, style).await,
            Self::Local(p) => p.translate(image_data, target_lang, style).await,
            #[cfg(test)]
            Self::Mock(p) => p.translate(image_data, target_lang, style).await,
        }
    }
}

pub fn create_provider(profile: &Profile) -> Result<AnyProvider> {
    match profile.driver.as_str() {
        "openai_compatible" => {
            let api_key = profile.resolve_api_key()?.ok_or_else(|| {
                anyhow::anyhow!("api_key_env is required for openai_compatible driver")
            })?;
            Ok(AnyProvider::Cloud(cloud::CloudProvider::new(
                api_key,
                profile.base_url.clone(),
                profile.model.clone(),
                profile.timeout_sec,
            )))
        }
        "ollama" => Ok(AnyProvider::Local(local::LocalProvider::new(
            profile.base_url.clone(),
            profile.model.clone(),
            profile.timeout_sec,
        ))),
        driver => anyhow::bail!("Unknown driver: {driver}"),
    }
}
