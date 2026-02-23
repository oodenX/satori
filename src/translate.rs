use anyhow::{Context, Result};

use crate::provider::{AnyProvider, TranslationResult};

/// Translate raw image bytes (from screenshot or file).
pub fn translate_bytes(
    provider: &AnyProvider,
    image_data: &[u8],
    target_lang: &str,
    style: &str,
) -> Result<TranslationResult> {
    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    let result = rt
        .block_on(provider.translate(image_data, target_lang, style))
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(result)
}
