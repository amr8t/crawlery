//! Content extraction module for RAG/LLM use cases.
//!
//! This module provides readability-style content extraction that converts HTML
//! into clean, structured markdown suitable for LLM ingestion and RAG applications.
//! It identifies the main content area, removes boilerplate (navigation, ads, footers),
//! and preserves document structure.
//!
//! # Features
//!
//! - Readability-style content scoring to identify main content
//! - Automatic removal of navigation, ads, footers, and other boilerplate
//! - Clean markdown conversion with proper structure preservation
//! - Metadata extraction (title, author, date, description)
//! - HTML entity decoding and whitespace normalization
//!
//! # Examples
//!
//! ```no_run
//! use crawlery::content::{extract_content, extract_metadata};
//!
//! let html = r#"
//!     <html>
//!         <head>
//!             <title>My Article</title>
//!             <meta name="description" content="A great article">
//!         </head>
//!         <body>
//!             <nav>Navigation</nav>
//!             <article>
//!                 <h1>Main Title</h1>
//!                 <p>This is the main content.</p>
//!             </article>
//!             <footer>Copyright 2024</footer>
//!         </body>
//!     </html>
//! "#;
//!
//! // Extract clean markdown
//! let markdown = extract_content(html)?;
//! println!("{}", markdown);
//!
//! // Extract metadata
//! let metadata = extract_metadata(html);
//! println!("Title: {}", metadata.get("title").unwrap_or(&"".to_string()));
//! # Ok::<(), anyhow::Error>(())
//! ```

use anyhow::Result;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashMap;

/// Extracts the main content from HTML and converts it to clean markdown.
///
/// This function uses a readability-style algorithm to identify the main content
/// area of a web page, removing navigation, ads, footers, and other boilerplate.
/// The result is clean markdown suitable for LLM ingestion.
///
/// # Algorithm
///
/// 1. Parse HTML into a DOM tree
/// 2. Remove unwanted elements (scripts, styles, ads, navigation)
/// 3. Score remaining elements based on content quality indicators
/// 4. Select the highest-scoring content container
/// 5. Convert to markdown while preserving structure
/// 6. Clean and normalize the output
///
/// # Arguments
///
/// * `html` - The HTML content to extract from
///
/// # Returns
///
/// Returns clean markdown text suitable for RAG/LLM applications.
///
/// # Examples
///
/// ```no_run
/// use crawlery::content::extract_content;
///
/// let html = "<article><h1>Title</h1><p>Content here.</p></article>";
/// let markdown = extract_content(html)?;
/// assert!(markdown.contains("# Title"));
/// assert!(markdown.contains("Content here"));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn extract_content(html: &str) -> Result<String> {
    let document = Html::parse_document(html);

    // Try to find main content using multiple strategies
    let content_html =
        find_main_content(&document).unwrap_or_else(|| extract_fallback_content(&document));

    // Convert to markdown
    let markdown = html2md::parse_html(&content_html);

    // Clean up the markdown
    let cleaned = clean_markdown(&markdown);

    Ok(cleaned)
}

/// Extracts metadata from HTML including title, author, date, and description.
///
/// This function searches for common metadata patterns in HTML, including:
/// - `<title>` tag
/// - Open Graph tags (`og:title`, `og:description`, etc.)
/// - Twitter Card tags
/// - Schema.org markup
/// - Meta tags (author, description, date)
/// - Article-specific tags
///
/// # Arguments
///
/// * `html` - The HTML content to extract metadata from
///
/// # Returns
///
/// Returns a HashMap with available metadata. Common keys include:
/// - `title`: Page or article title
/// - `description`: Page description
/// - `author`: Article author
/// - `date`: Publication date
/// - `site_name`: Website name
/// - `image`: Featured image URL
///
/// # Examples
///
/// ```no_run
/// use crawlery::content::extract_metadata;
///
/// let html = r#"<html><head><title>My Page</title></head></html>"#;
/// let metadata = extract_metadata(html);
/// assert_eq!(metadata.get("title").map(|s| s.as_str()), Some("My Page"));
/// ```
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

/// Finds the main content area using readability-style scoring.
fn find_main_content(document: &Html) -> Option<String> {
    let mut best_score = 0.0;
    let mut best_element: Option<ElementRef> = None;

    // Try semantic HTML5 elements first with scoring
    let semantic_selectors = ["article", "main", "[role='main']"];

    for selector_str in &semantic_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                let html = element_to_html(&element);
                if is_substantial_content(&html) {
                    let score = score_element(&element) + 50.0; // Bonus for semantic elements
                    if score > best_score {
                        best_score = score;
                        best_element = Some(element);
                    }
                }
            }
        }
    }

    // Also check divs and sections if we haven't found good semantic content
    if best_score < 75.0 {
        if let Ok(div_selector) = Selector::parse("div, section") {
            for element in document.select(&div_selector) {
                let score = score_element(&element);
                if score > best_score {
                    best_score = score;
                    best_element = Some(element);
                }
            }
        }
    }

    // Return the best element if it has a good enough score
    if let Some(element) = best_element {
        if best_score > 10.0 {
            return Some(element_to_html(&element));
        }
    }

    None
}

/// Scores an element based on content quality indicators.
fn score_element(element: &ElementRef) -> f64 {
    let mut score = 0.0;

    // Check if element should be ignored
    if should_ignore_element(element) {
        return 0.0;
    }

    // Get text content
    let text: String = element.text().collect();
    let text_length = text.len();

    // Base score from text length
    score += (text_length as f64) / 100.0;

    // Count paragraphs
    if let Ok(p_selector) = Selector::parse("p") {
        let p_count = element.select(&p_selector).count();
        score += (p_count as f64) * 3.0;
    }

    // Bonus for semantic content elements
    if let Ok(content_selector) = Selector::parse("article, section, p, h1, h2, h3, h4, h5, h6") {
        let content_elements = element.select(&content_selector).count();
        score += (content_elements as f64) * 2.0;
    }

    // Check class and id for positive indicators
    if let Some(class) = element.value().attr("class") {
        if class.contains("content")
            || class.contains("article")
            || class.contains("post")
            || class.contains("entry")
            || class.contains("main")
        {
            score += 25.0;
        }
        if class.contains("comment") || class.contains("footer") || class.contains("sidebar") {
            score -= 25.0;
        }
    }

    if let Some(id) = element.value().attr("id") {
        if id.contains("content")
            || id.contains("article")
            || id.contains("post")
            || id.contains("entry")
            || id.contains("main")
        {
            score += 25.0;
        }
        if id.contains("comment") || id.contains("footer") || id.contains("sidebar") {
            score -= 25.0;
        }
    }

    // Penalty for high link density
    if let Ok(a_selector) = Selector::parse("a") {
        let link_text: String = element
            .select(&a_selector)
            .flat_map(|el| el.text())
            .collect();
        let link_density = if text_length > 0 {
            link_text.len() as f64 / text_length as f64
        } else {
            0.0
        };

        if link_density > 0.5 {
            score *= 0.5;
        }
    }

    score.max(0.0)
}

/// Checks if an element should be ignored based on tag or attributes.
fn should_ignore_element(element: &ElementRef) -> bool {
    let tag = element.value().name();

    // Ignore non-content tags
    if matches!(
        tag,
        "script" | "style" | "nav" | "header" | "footer" | "aside" | "iframe" | "noscript" | "form"
    ) {
        return true;
    }

    // Ignore based on class or id
    if let Some(class) = element.value().attr("class") {
        if class.contains("nav")
            || class.contains("menu")
            || class.contains("sidebar")
            || class.contains("ad")
            || class.contains("advertisement")
            || class.contains("promo")
            || class.contains("social")
            || class.contains("share")
            || class.contains("cookie")
            || class.contains("modal")
            || class.contains("popup")
        {
            return true;
        }
    }

    if let Some(id) = element.value().attr("id") {
        if id.contains("nav")
            || id.contains("menu")
            || id.contains("sidebar")
            || id.contains("ad")
            || id.contains("footer")
            || id.contains("header")
        {
            return true;
        }
    }

    // Ignore hidden elements
    if let Some(style) = element.value().attr("style") {
        if style.contains("display:none")
            || style.contains("display: none")
            || style.contains("visibility:hidden")
            || style.contains("visibility: hidden")
        {
            return true;
        }
    }

    false
}

/// Extracts fallback content when main content cannot be identified.
fn extract_fallback_content(document: &Html) -> String {
    // Try body element
    if let Ok(body_selector) = Selector::parse("body") {
        if let Some(body) = document.select(&body_selector).next() {
            return element_to_html(&body);
        }
    }

    // Last resort: entire document
    document.html()
}

/// Converts an element to HTML string, filtering out unwanted elements.
fn element_to_html(element: &ElementRef) -> String {
    let html = element.html();

    // Remove unwanted elements using regex to avoid duplication
    let mut cleaned = html;

    let removal_patterns = [
        r"(?s)<script[^>]*>.*?</script>",
        r"(?s)<style[^>]*>.*?</style>",
        r"(?s)<nav[^>]*>.*?</nav>",
        r"(?s)<header[^>]*>.*?</header>",
        r"(?s)<footer[^>]*>.*?</footer>",
        r"(?s)<aside[^>]*>.*?</aside>",
        r"(?s)<iframe[^>]*>.*?</iframe>",
        r"(?s)<noscript[^>]*>.*?</noscript>",
        r"(?s)<form[^>]*>.*?</form>",
        r#"(?s)<div[^>]*class="[^"]*\b(ad|advertisement|promo|sidebar|cookie|modal|popup)\b[^"]*"[^>]*>.*?</div>"#,
        r#"(?s)<div[^>]*id="[^"]*\b(ad|advertisement|promo|sidebar|cookie|modal|popup)\b[^"]*"[^>]*>.*?</div>"#,
    ];

    for pattern in &removal_patterns {
        if let Ok(re) = Regex::new(pattern) {
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
    }

    cleaned
}

/// Checks if content is substantial enough to be main content.
fn is_substantial_content(html: &str) -> bool {
    let text = strip_html_tags(html);
    let word_count = text.split_whitespace().count();
    word_count >= 50 // At least 50 words
}

/// Strips HTML tags from a string.
fn strip_html_tags(html: &str) -> String {
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    tag_re.replace_all(html, " ").to_string()
}

/// Extracts the page title from various sources.
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

/// Cleans and normalizes markdown output.
fn clean_markdown(markdown: &str) -> String {
    let mut cleaned = markdown.to_string();

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
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&copy;", "©")
        .replace("&reg;", "®")
        .replace("&trade;", "™");

    // Clean up whitespace
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
    fn test_extract_content_basic() {
        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <nav>Skip this</nav>
                    <article>
                        <h1>Main Title</h1>
                        <p>This is the main content.</p>
                    </article>
                    <footer>Copyright</footer>
                </body>
            </html>
        "#;

        let result = extract_content(html).unwrap();
        assert!(result.contains("Main Title"));
        assert!(result.contains("main content"));
        assert!(!result.contains("Skip this"));
        assert!(!result.contains("Copyright"));
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
        let markdown = "# Title\n\n\n\nContent&nbsp;here\n\n\n";
        let cleaned = clean_markdown(markdown);
        assert_eq!(cleaned, "# Title\n\nContent here\n");
    }

    #[test]
    fn test_score_element() {
        let html = r#"<div class="content"><p>Paragraph 1</p><p>Paragraph 2</p></div>"#;
        let document = Html::parse_fragment(html);
        if let Ok(selector) = Selector::parse("div") {
            if let Some(element) = document.select(&selector).next() {
                let score = score_element(&element);
                assert!(score > 0.0);
            }
        }
    }

    #[test]
    fn test_should_ignore_element() {
        let html = r#"<nav>Navigation</nav>"#;
        let document = Html::parse_fragment(html);
        if let Ok(selector) = Selector::parse("nav") {
            if let Some(element) = document.select(&selector).next() {
                assert!(should_ignore_element(&element));
            }
        }
    }
}
