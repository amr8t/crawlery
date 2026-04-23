//! Output formatting module for Crawlery.
//!
//! This module provides functions to save crawl results in various formats,
//! optimized for different use cases such as RAG (Retrieval-Augmented Generation),
//! data analysis, and human readability.
//!
//! # Supported Formats
//!
//! - **JSON** - Compact JSON for programmatic processing
//! - **JSON Pretty** - Human-readable JSON with indentation
//! - **Markdown** - RAG-friendly format with structured content
//! - **CSV** - Tabular format for spreadsheets and data analysis
//! - **Text** - Plain text format for easy reading
//!
//! # Examples
//!
//! ```no_run
//! use crawlery::{CrawlResult, OutputFormat};
//! use crawlery::output::save_results;
//! use std::path::PathBuf;
//!
//! # fn main() -> anyhow::Result<()> {
//! let results = vec![
//!     CrawlResult::new("https://example.com".to_string(), "Content here".to_string(), 0),
//! ];
//!
//! // Save as JSON
//! save_results(&results, OutputFormat::JsonPretty, Some(PathBuf::from("output.json")))?;
//!
//! // Save as Markdown for RAG
//! save_results(&results, OutputFormat::Markdown, Some(PathBuf::from("output.md")))?;
//!
//! // Print to stdout
//! save_results(&results, OutputFormat::Text, None)?;
//! # Ok(())
//! # }
//! ```

use crate::{CrawlResult, OutputFormat};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

/// Saves crawl results in the specified format.
///
/// This is the main entry point for saving crawl results. It dispatches to
/// format-specific functions based on the `OutputFormat` parameter.
///
/// # Arguments
///
/// * `results` - Slice of crawl results to save
/// * `format` - The output format to use (JSON, Markdown, CSV, or Text)
/// * `path` - Optional file path; if `None`, output goes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing fails.
///
/// # Examples
///
/// ```no_run
/// use crawlery::{CrawlResult, OutputFormat};
/// use crawlery::output::save_results;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let results = vec![
///     CrawlResult::new("https://example.com".to_string(), "Content".to_string(), 0),
/// ];
///
/// // Save to file
/// save_results(&results, OutputFormat::Json, Some(PathBuf::from("results.json")))?;
///
/// // Print to stdout
/// save_results(&results, OutputFormat::Text, None)?;
/// # Ok(())
/// # }
/// ```
pub fn save_results(
    results: &[CrawlResult],
    format: OutputFormat,
    path: Option<PathBuf>,
) -> Result<()> {
    match format {
        OutputFormat::Json => save_json(results, path),
        OutputFormat::JsonPretty => save_json_pretty(results, path),
        OutputFormat::Markdown => save_markdown(results, path),
        OutputFormat::Csv => save_csv(results, path),
        OutputFormat::Text => save_text(results, path),
    }
}

/// Saves results as compact JSON.
///
/// Serializes the results to compact JSON format without indentation.
/// This format produces smaller files but is less human-readable.
///
/// # Arguments
///
/// * `results` - Slice of crawl results to save
/// * `path` - Optional file path; if `None`, output goes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if serialization or writing fails.
///
/// # Examples
///
/// ```no_run
/// use crawlery::CrawlResult;
/// use crawlery::output::save_json;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let results = vec![
///     CrawlResult::new("https://example.com".to_string(), "Content".to_string(), 0),
/// ];
///
/// save_json(&results, Some(PathBuf::from("compact.json")))?;
/// # Ok(())
/// # }
/// ```
pub fn save_json(results: &[CrawlResult], path: Option<PathBuf>) -> Result<()> {
    let json = serde_json::to_string(results).context("Failed to serialize to JSON")?;
    write_output(&json, path)
}

/// Saves results as pretty-printed JSON.
///
/// Serializes the results to human-readable JSON format with indentation.
/// This format is easier to read and debug but produces larger files.
///
/// # Arguments
///
/// * `results` - Slice of crawl results to save
/// * `path` - Optional file path; if `None`, output goes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if serialization or writing fails.
///
/// # Examples
///
/// ```no_run
/// use crawlery::CrawlResult;
/// use crawlery::output::save_json_pretty;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let results = vec![
///     CrawlResult::new("https://example.com".to_string(), "Content".to_string(), 0),
/// ];
///
/// save_json_pretty(&results, Some(PathBuf::from("readable.json")))?;
/// # Ok(())
/// # }
/// ```
pub fn save_json_pretty(results: &[CrawlResult], path: Option<PathBuf>) -> Result<()> {
    let json = serde_json::to_string_pretty(results).context("Failed to serialize to JSON")?;
    write_output(&json, path)
}

/// Saves results as markdown format (RAG-friendly).
///
/// Converts crawl results to well-structured Markdown format, optimized for
/// Retrieval-Augmented Generation (RAG) applications. Each result includes:
/// - URL and metadata headers
/// - Content section with cleaned text
/// - Links section with discovered URLs
/// - Errors section (if any occurred)
///
/// # Arguments
///
/// * `results` - Slice of crawl results to save
/// * `path` - Optional file path; if `None`, output goes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing fails.
///
/// # Examples
///
/// ```no_run
/// use crawlery::CrawlResult;
/// use crawlery::output::save_markdown;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let results = vec![
///     CrawlResult::new("https://example.com".to_string(), "Page content".to_string(), 0),
/// ];
///
/// save_markdown(&results, Some(PathBuf::from("crawl.md")))?;
/// # Ok(())
/// # }
/// ```
pub fn save_markdown(results: &[CrawlResult], path: Option<PathBuf>) -> Result<()> {
    let mut output = String::new();

    for result in results {
        output.push_str(&format!("# URL: {}\n", result.url));

        if let Some(status) = result.status_code {
            output.push_str(&format!("Status: {}\n", status));
        }

        output.push_str(&format!("Depth: {}\n", result.depth));

        if let Some(title) = &result.title {
            output.push_str(&format!("Title: {}\n", title));
        }

        output.push_str("\n## Content\n");
        output.push_str(&result.content);
        output.push_str("\n\n");

        if !result.links.is_empty() {
            output.push_str("## Links\n");
            for link in &result.links {
                output.push_str(&format!("- {}\n", link));
            }
            output.push('\n');
        }

        if !result.errors.is_empty() {
            output.push_str("## Errors\n");
            for error in &result.errors {
                output.push_str(&format!("- {}\n", error));
            }
            output.push('\n');
        }

        output.push_str("---\n\n");
    }

    write_output(&output, path)
}

/// Saves results as CSV format.
///
/// Converts crawl results to comma-separated values (CSV) format suitable for
/// spreadsheet applications and data analysis tools. Includes columns for:
/// - URL, status code, title, depth
/// - Link count, content length, content type
/// - Errors (semicolon-separated)
///
/// # Arguments
///
/// * `results` - Slice of crawl results to save
/// * `path` - Optional file path; if `None`, output goes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing fails.
///
/// # Examples
///
/// ```no_run
/// use crawlery::CrawlResult;
/// use crawlery::output::save_csv;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let results = vec![
///     CrawlResult::new("https://example.com".to_string(), "Content".to_string(), 0),
/// ];
///
/// save_csv(&results, Some(PathBuf::from("crawl.csv")))?;
/// # Ok(())
/// # }
/// ```
pub fn save_csv(results: &[CrawlResult], path: Option<PathBuf>) -> Result<()> {
    let mut output = String::new();

    // CSV header
    output.push_str("url,status_code,title,depth,link_count,content_length,content_type,errors\n");

    // CSV rows
    for result in results {
        let status = result.status_code.map_or(String::new(), |s| s.to_string());
        let title = result.title.as_deref().unwrap_or("");
        let content_type = result.content_type.as_deref().unwrap_or("");
        let link_count = result.links.len();
        let content_length = result.content.len();
        let errors = result.errors.join("; ");

        output.push_str(&format!(
            "\"{}\",{},\"{}\",{},{},{},\"{}\",\"{}\"\n",
            escape_csv(&result.url),
            status,
            escape_csv(title),
            result.depth,
            link_count,
            content_length,
            escape_csv(content_type),
            escape_csv(&errors)
        ));
    }

    write_output(&output, path)
}

/// Saves results as plain text format.
///
/// Converts crawl results to human-readable plain text format with clear
/// section separators. Each result includes all available information in
/// an easy-to-read layout.
///
/// # Arguments
///
/// * `results` - Slice of crawl results to save
/// * `path` - Optional file path; if `None`, output goes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing fails.
///
/// # Examples
///
/// ```no_run
/// use crawlery::CrawlResult;
/// use crawlery::output::save_text;
/// use std::path::PathBuf;
///
/// # fn main() -> anyhow::Result<()> {
/// let results = vec![
///     CrawlResult::new("https://example.com".to_string(), "Content".to_string(), 0),
/// ];
///
/// save_text(&results, Some(PathBuf::from("crawl.txt")))?;
/// # Ok(())
/// # }
/// ```
pub fn save_text(results: &[CrawlResult], path: Option<PathBuf>) -> Result<()> {
    let mut output = String::new();

    for (i, result) in results.iter().enumerate() {
        output.push_str(&format!("========== Result {} ==========\n", i + 1));
        output.push_str(&format!("URL: {}\n", result.url));

        if let Some(status) = result.status_code {
            output.push_str(&format!("Status: {}\n", status));
        }

        if let Some(title) = &result.title {
            output.push_str(&format!("Title: {}\n", title));
        }

        output.push_str(&format!("Depth: {}\n", result.depth));
        output.push_str(&format!("Links found: {}\n", result.links.len()));

        if let Some(content_type) = &result.content_type {
            output.push_str(&format!("Content-Type: {}\n", content_type));
        }

        if !result.errors.is_empty() {
            output.push_str(&format!("Errors: {}\n", result.errors.join(", ")));
        }

        output.push_str(&format!("\nContent ({} chars):\n", result.content.len()));
        output.push_str(&result.content);
        output.push_str("\n\n");

        if !result.links.is_empty() {
            output.push_str("Links:\n");
            for link in &result.links {
                output.push_str(&format!("  - {}\n", link));
            }
            output.push('\n');
        }

        output.push('\n');
    }

    write_output(&output, path)
}

/// Writes output to a file or stdout.
///
/// Internal helper function that handles the actual writing of content to either
/// a file or stdout, depending on whether a path is provided.
///
/// # Arguments
///
/// * `content` - The string content to write
/// * `path` - Optional file path; if `None`, writes to stdout
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if writing fails.
fn write_output(content: &str, path: Option<PathBuf>) -> Result<()> {
    match path {
        Some(path) => {
            let mut file = File::create(&path)
                .context(format!("Failed to create file: {}", path.display()))?;
            file.write_all(content.as_bytes())
                .context(format!("Failed to write to file: {}", path.display()))?;
            println!("Results saved to: {}", path.display());
        }
        None => {
            io::stdout()
                .write_all(content.as_bytes())
                .context("Failed to write to stdout")?;
        }
    }
    Ok(())
}

/// Escapes CSV field content by replacing quotes with double quotes.
///
/// This helper function properly escapes double quotes in CSV fields according
/// to RFC 4180 by replacing each `"` with `""`.
fn escape_csv(s: &str) -> String {
    s.replace('"', "\"\"")
}


/// Save projected (field-filtered) results as JSON.
pub fn save_projected(values: &[serde_json::Value], path: Option<PathBuf>) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(values).context("Failed to serialize projected results")?;
    write_output(&json, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("hello"), "hello");
        assert_eq!(escape_csv("hello \"world\""), "hello \"\"world\"\"");
    }
}
