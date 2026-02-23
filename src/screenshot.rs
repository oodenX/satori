use anyhow::{Context, Result};
use std::process::Command;

/// Run `slurp` to let the user select a screen region.
/// Returns the geometry string (e.g., "100,200 300x400").
pub fn select_region() -> Result<String> {
    let output = Command::new("slurp")
        .output()
        .context("Failed to run slurp. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("slurp exited with error: {stderr}");
    }

    let region = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 from slurp")?
        .trim()
        .to_string();

    Ok(region)
}

/// Capture a screenshot of the given region using `grim`.
/// Returns the PNG image data as in-memory bytes (never written to disk).
pub fn capture_region(geometry: &str) -> Result<Vec<u8>> {
    let output = Command::new("grim")
        .args(["-g", geometry, "-"])
        .output()
        .context("Failed to run grim. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("grim exited with error: {stderr}");
    }

    Ok(output.stdout)
}

/// Convenience: select region then capture.
pub fn take_screenshot() -> Result<Vec<u8>> {
    let region = select_region().context("Region selection failed")?;
    capture_region(&region).context("Screenshot capture failed")
}
