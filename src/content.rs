//! Content extraction module for RAG/LLM use cases.
//!
//! This module provides readability-style content extraction that converts HTML
//! into clean, structured markdown suitable for LLM ingestion and RAG applications.
//! It uses the Mozilla/arc90 Readability algorithm (the same engine as Firefox Reader
//! Mode) to identify and extract the main content of a page.
//!
//! # Features
//!
//! - Mozilla Readability algorithm to identify main content
//! - Automatic removal of navigation, ads, footers, and other boilerplate
//! - Clean markdown conversion via htmd (turndown.js-inspired)
//! - Metadata extraction (title, author, date, description)
//! - HTML entity decoding and whitespace normalization

use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashMap;

/// Extracts the main content from HTML and converts it to clean markdown.
///
/// Uses the Mozilla/arc90 Readability algorithm (same engine as Firefox Reader Mode)
/// to identify the main content area and strip boilerplate, then converts the
/// cleaned HTML to Markdown via htmd (a turndown.js-inspired converter).
///
/// # Arguments
///
/// *  - The raw HTML to extract content from.
/// *   - The URL the HTML was fetched from. Used by the Readability engine
///            to resolve relative links; falls back to  if invalid.
///
/// # Returns
///
/// Returns clean markdown text suitable for RAG/LLM applications.
pub fn md_readability(html: &str, url: &str) -> Result<String> {
    use readability::extractor;
    use url::Url;

    let parsed_url =
        Url::parse(url).unwrap_or_else(|_| Url::parse("http://localhost/").unwrap());

    // Try Mozilla Readability. It excels at article pages.
    // When it succeeds with meaningful content (> 200 chars of extracted HTML)
    // we use it; otherwise we fall back to htmd on the full page.
    // (Readability returns just the site name for table/list pages like HN,
    //  the same way Firefox shows "Reader View unavailable" for such pages.)
    let mut cursor = std::io::Cursor::new(html.as_bytes());
    if let Ok(product) = extractor::extract(&mut cursor, &parsed_url) {
        if product.content.trim().len() > 200 {
            let html_with_title = if product.title.is_empty() {
                product.content
            } else {
                format!("<h1>{}</h1>\n{}", product.title, product.content)
            };
            let markdown = htmd::convert(&html_with_title).unwrap_or_default();
            return Ok(clean_markdown(&markdown));
        }
    }

    // Fallback: convert the full page with htmd -- works well for list/nav pages.
    let markdown = htmd::convert(html).unwrap_or_else(|_| {
        Regex::new(r"<[^>]+>")
            .unwrap()
            .replace_all(html, " ")
            .to_string()
    });
    Ok(clean_markdown(&markdown))
}

pub fn extract_metadata(html: &str) -> HashMap<String, String> {
    let document = Html::parse_document(html);
    let mut metadata = HashMap::new();

    // Extract title
    if let Some(title) = extract_title(&document) {
        metadata.insert("title".to_string(), title);
    }

    // Extract meta tags
    if let Ok(meta_selector) = Selector::parse("meta") {
        for element in document.select(&meta_selector) {
            // OpenGraph tags
            if let Some(property) = element.value().attr("property") {
                if let Some(content) = element.value().attr("content") {
                    match property {
                        "og:title" => {
                            metadata.insert("title".to_string(), content.to_string());
                            &mut String::new()
                        }
                        "og:description" => {
                            metadata.insert("description".to_string(), content.to_string());
                            &mut String::new()
                        }
                        "og:site_name" => {
                            metadata.insert("site_name".to_string(), content.to_string());
                            &mut String::new()
                        }
                        "og:image" => {
                            metadata.insert("image".to_string(), content.to_string());
                            &mut String::new()
                        }
                        "article:author" => {
                            metadata.insert("author".to_string(), content.to_string());
                            &mut String::new()
                        }
                        "article:published_time" => {
                            metadata.insert("date".to_string(), content.to_string());
                            &mut String::new()
                        }
                        _ => &mut String::new(),
                    };
                }
            }

            // Standard meta tags
            if let Some(name) = element.value().attr("name") {
                if let Some(content) = element.value().attr("content") {
                    match name {
                        "description" => metadata
                            .entry("description".to_string())
                            .or_insert(content.to_string()),
                        "author" => metadata
                            .entry("author".to_string())
                            .or_insert(content.to_string()),
                        "date" | "publish-date" | "publication-date" => metadata
                            .entry("date".to_string())
                            .or_insert(content.to_string()),
                        "twitter:title" => metadata
                            .entry("title".to_string())
                            .or_insert(content.to_string()),
                        "twitter:description" => metadata
                            .entry("description".to_string())
                            .or_insert(content.to_string()),
                        "twitter:image" => metadata
                            .entry("image".to_string())
                            .or_insert(content.to_string()),
                        _ => &mut String::new(),
                    };
                }
            }
        }
    }

    // Extract author from common selectors
    if !metadata.contains_key("author") {
        let author_selectors = [
            ".author",
            ".byline",
            "[rel='author']",
            "[itemprop='author']",
            ".post-author",
            ".entry-author",
            ".article-author",
        ];

        for selector_str in &author_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let author = element.text().collect::<String>().trim().to_string();
                    if !author.is_empty() {
                        metadata.insert("author".to_string(), author);
                        break;
                    }
                }
            }
        }
    }

    // Extract date from common selectors
    if !metadata.contains_key("date") {
        let date_selectors = [
            "time",
            ".published",
            ".date",
            ".post-date",
            ".entry-date",
            "[itemprop='datePublished']",
            "[itemprop='dateModified']",
        ];

        for selector_str in &date_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    // Try datetime attribute first
                    if let Some(datetime) = element.value().attr("datetime") {
                        metadata.insert("date".to_string(), datetime.to_string());
                        break;
                    }
                    // Fall back to text content
                    let date = element.text().collect::<String>().trim().to_string();
                    if !date.is_empty() {
                        metadata.insert("date".to_string(), date);
                        break;
                    }
                }
            }
        }
    }

    metadata
}
fn extract_title(document: &Html) -> Option<String> {
    // Try h1 first
    if let Ok(h1_selector) = Selector::parse("h1") {
        if let Some(h1) = document.select(&h1_selector).next() {
            let title = h1.text().collect::<String>().trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }

    // Try title tag
    if let Ok(title_selector) = Selector::parse("title") {
        if let Some(title_elem) = document.select(&title_selector).next() {
            let title = title_elem.text().collect::<String>().trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }

    None
}

fn clean_markdown(markdown: &str) -> String {
    let mut cleaned = markdown.to_string();

    // Strip any residual HTML tags (e.g. <br>, <img>) that htmd may leave behind.
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    cleaned = tag_re.replace_all(&cleaned, "").to_string();

    // Strip whitespace-only table rows (artifacts of converting layout tables).
    // A row like "|     |     |" contains no real content.
    let empty_row_re = Regex::new(r"(?m)^\|[\s|]*$").unwrap();
    cleaned = empty_row_re.replace_all(&cleaned, "").to_string();

    // Collapse runs of spaces/tabs within a line to a single space.
    // htmd may produce wide spacing in table cells; this compacts them.
    let space_re = Regex::new(r"[ \t]{2,}").unwrap();
    cleaned = space_re.replace_all(&cleaned, " ").to_string();

    // Remove excessive blank lines (more than 2 consecutive)
    let blank_lines_re = Regex::new(r"\n{3,}").unwrap();
    cleaned = blank_lines_re.replace_all(&cleaned, "\n\n").to_string();

    // Decode common HTML entities that might remain
    cleaned = cleaned
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&mdash;", "\u{2014}")
        .replace("&ndash;", "\u{2013}")
        .replace("&hellip;", "\u{2026}")
        .replace("&copy;", "\u{00a9}")
        .replace("&reg;", "\u{00ae}")
        .replace("&trade;", "\u{2122}");

    // Trim trailing whitespace from each line
    let lines: Vec<String> = cleaned
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();

    let mut result = lines.join("\n");

    // Ensure single trailing newline
    result = result.trim().to_string();
    result.push('\n');

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md_readability_basic() {
        // Readability requires a minimum content length to fire; give it a real article.
        let html = r#"
            <html>
                <head><title>Main Title</title></head>
                <body>
                    <nav>Skip this nav</nav>
                    <article>
                        <h1>Main Title</h1>
                        <p>This is the main content of the article. Web crawlers are programs
                        that systematically browse the World Wide Web, indexing content for
                        search engines and other applications. They follow hyperlinks from
                        page to page, downloading and processing the HTML they find.</p>
                        <p>A good crawler respects robots.txt, handles errors gracefully, and
                        avoids overloading target servers. This article explains how modern
                        crawlers work and what best practices look like in production systems.</p>
                        <p>Content extraction is a key part of the pipeline: once the HTML is
                        downloaded, the crawler must identify the main body text and discard
                        boilerplate such as navigation menus, advertisements, and footers.</p>
                    </article>
                    <footer>Copyright notice here</footer>
                </body>
            </html>
        "#;

        let result = md_readability(html, "http://localhost/").unwrap();
        assert!(result.contains("Main Title"), "should contain heading");
        assert!(result.contains("main content"), "should contain article text");
    }

    #[test]
    fn test_extract_metadata() {
        let html = r#"
            <html>
                <head>
                    <title>Test Article</title>
                    <meta name="description" content="A test description">
                    <meta name="author" content="John Doe">
                    <meta property="og:title" content="OG Title">
                </head>
            </html>
        "#;

        let metadata = extract_metadata(html);
        assert_eq!(metadata.get("title").map(|s| s.as_str()), Some("OG Title"));
        assert_eq!(
            metadata.get("description").map(|s| s.as_str()),
            Some("A test description")
        );
        assert_eq!(metadata.get("author").map(|s| s.as_str()), Some("John Doe"));
    }

    #[test]
    fn test_clean_markdown() {
        // Existing behaviour: excessive blank lines collapsed, entities decoded
        let markdown = "# Title\n\n\n\nContent&nbsp;here\n\n\n";
        let cleaned = clean_markdown(markdown);
        assert_eq!(cleaned, "# Title\n\nContent here\n");

        // HTML tags are stripped
        let with_tag = "hello <br> world";
        let cleaned_tag = clean_markdown(with_tag);
        assert!(!cleaned_tag.contains('<'), "HTML tags should be stripped");

        // Whitespace-only table rows are removed
        assert_eq!(clean_markdown("|\t|\n").trim(), "");

        // Runs of spaces/tabs within a line are collapsed
        assert!(
            !clean_markdown("a   b").contains("   "),
            "triple spaces should be collapsed"
        );
    }
}

