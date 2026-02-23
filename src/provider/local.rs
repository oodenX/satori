use reqwest::Client;
use serde_json::json;
use std::time::Duration;

use crate::prompt;

use super::{ProviderError, TranslationResult};

pub struct LocalProvider {
    base_url: String,
    model: String,
    timeout: Duration,
    client: Client,
}

impl LocalProvider {
    pub fn new(base_url: String, model: String, timeout_sec: u64) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_sec))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            base_url,
            model,
            timeout: Duration::from_secs(timeout_sec),
            client,
        }
    }
}

impl LocalProvider {
    pub async fn translate(
        &self,
        image_data: &[u8],
        target_lang: &str,
        style: &str,
    ) -> Result<TranslationResult, ProviderError> {
        let b64_image =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image_data);
        let system_prompt = prompt::system_prompt(target_lang, style);

        let body = json!({
            "model": self.model,
            "format": "json",
            "system": system_prompt,
            "prompt": format!("Translate the text in this image to {target_lang}."),
            "images": [b64_image],
            "stream": false,
        });

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(self.timeout.as_secs())
                } else {
                    ProviderError::RequestFailed(e.to_string())
                }
            })?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        if !status.is_success() {
            return Err(ProviderError::RequestFailed(format!(
                "HTTP {status}: {response_text}"
            )));
        }

        let api_response: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        let content = api_response["response"]
            .as_str()
            .ok_or_else(|| ProviderError::ParseError("Missing 'response' field".to_string()))?;

        let translation = prompt::extract_json(content).map_err(ProviderError::ParseError)?;

        Ok(TranslationResult {
            source_text: translation.source_text,
            translated_text: translation.translated_text,
        })
    }
}
