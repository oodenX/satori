use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub source_text: String,
    pub translated_text: String,
    pub target_lang: String,
    pub style: String,
}

fn history_path() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME environment variable not set");
            PathBuf::from(home).join(".local/share")
        });
    base.join("satori").join("history.jsonl")
}

/// Append a translation result to the history file.
pub fn record(
    source_text: &str,
    translated_text: &str,
    target_lang: &str,
    style: &str,
) -> Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let entry = HistoryEntry {
        timestamp: chrono_like_now(),
        source_text: source_text.to_string(),
        translated_text: translated_text.to_string(),
        target_lang: target_lang.to_string(),
        style: style.to_string(),
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open {}", path.display()))?;

    let json = serde_json::to_string(&entry).context("Failed to serialize history entry")?;
    writeln!(file, "{json}").context("Failed to write history entry")?;

    Ok(())
}

/// Read all history entries (most recent last).
pub fn list_entries(limit: Option<usize>) -> Result<Vec<HistoryEntry>> {
    let path = history_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file =
        fs::File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut entries: Vec<HistoryEntry> = reader
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            serde_json::from_str(&line).ok()
        })
        .collect();

    if let Some(n) = limit {
        let skip = entries.len().saturating_sub(n);
        entries = entries.into_iter().skip(skip).collect();
    }

    Ok(entries)
}

/// Search history entries by keyword (case-insensitive).
pub fn search_entries(query: &str) -> Result<Vec<HistoryEntry>> {
    let query_lower = query.to_lowercase();
    let entries = list_entries(None)?;
    Ok(entries
        .into_iter()
        .filter(|e| {
            e.source_text.to_lowercase().contains(&query_lower)
                || e.translated_text.to_lowercase().contains(&query_lower)
        })
        .collect())
}

/// Clear all history.
pub fn clear() -> Result<()> {
    let path = history_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

/// Simple ISO 8601-ish timestamp without pulling in chrono.
fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Format as Unix timestamp (lightweight, no chrono dependency)
    format!("{secs}")
}

/// Print history entries to stdout.
pub fn print_entries(entries: &[HistoryEntry]) {
    if entries.is_empty() {
        println!("No history entries found.");
        return;
    }
    for entry in entries {
        println!("─────────────────────────────────");
        println!("  Time: {}", entry.timestamp);
        println!("  Lang: {} ({})", entry.target_lang, entry.style);
        if !entry.source_text.is_empty() {
            println!("  Source: {}", entry.source_text);
        }
        if !entry.translated_text.is_empty() {
            println!("  Translation: {}", entry.translated_text);
        }
    }
    println!("─────────────────────────────────");
    println!("{} entries", entries.len());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_history_entry() {
        let json = r#"{"timestamp":"1700000000","source_text":"hello","translated_text":"你好","target_lang":"简体中文","style":"general"}"#;
        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.source_text, "hello");
        assert_eq!(entry.translated_text, "你好");
    }

    #[test]
    fn list_from_file() {
        let dir = std::env::temp_dir().join("satori-test-history");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.jsonl");

        let mut f = fs::File::create(&path).unwrap();
        for i in 0..5 {
            let entry = HistoryEntry {
                timestamp: format!("{i}"),
                source_text: format!("src{i}"),
                translated_text: format!("tgt{i}"),
                target_lang: "English".to_string(),
                style: "general".to_string(),
            };
            writeln!(f, "{}", serde_json::to_string(&entry).unwrap()).unwrap();
        }
        drop(f);

        // Read back
        let file = fs::File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let entries: Vec<HistoryEntry> = reader
            .lines()
            .filter_map(|l| serde_json::from_str(&l.ok()?).ok())
            .collect();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[2].source_text, "src2");

        let _ = fs::remove_dir_all(&dir);
    }
}
