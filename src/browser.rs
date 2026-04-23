//! Browser automation module for JavaScript-heavy web crawling.
//!
//! This module provides a headless Chrome-based crawler for scraping websites that rely
//! heavily on JavaScript to render content. It uses the `headless_chrome` crate to automate
//! a real browser, enabling crawling of dynamic single-page applications (SPAs) and sites
//! that require JavaScript execution.
//!
//! # Use Cases
//!
//! - Crawling JavaScript-rendered content (React, Vue, Angular apps)
//! - Sites that require user interaction simulation
//! - Pages that load content dynamically via AJAX
//! - Screenshots and visual testing
//! - Extracting data from complex web applications
//!
//! # Performance Considerations
//!
//! Browser automation is slower and more resource-intensive than HTTP crawling.
//! Use this mode only when necessary (i.e., when content is not available via HTTP).
//!
//! # Examples
//!
//! ```no_run
//! use crawlery::browser::{BrowserCrawler, BrowserConfig};
//!
//! fn main() -> anyhow::Result<()> {
//!     let config = BrowserConfig {
//!         proxy: None,
//!         user_agent: Some("MyBot/1.0".to_string()),
//!         timeout_secs: 30,
//!         headless: true,
//!         ..BrowserConfig::default()
//!     };
//!
//!     let crawler = BrowserCrawler::new(config)?;
//!     let result = crawler.fetch("https://example.com")?;
//!
//!     println!("Fetched {} with {} links", result.url, result.links.len());
//!     println!("Content: {}", result.cleaned_content);
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use std::sync::Arc;
use headless_chrome::{Browser, LaunchOptions};
use scraper::{Html, Selector};

use url::Url;

use crate::content;

/// Configuration for the browser-based crawler.
///
/// This struct contains settings for launching and controlling the headless Chrome browser,
/// including proxy settings, user agent customization, timeouts, and display mode.
///
/// # Examples
///
/// ```
/// use crawlery::browser::BrowserConfig;
///
/// // Use default configuration
/// let config = BrowserConfig::default();
///
/// // Or customize settings
/// let config = BrowserConfig {
///     proxy: Some("http://proxy.example.com:8080".to_string()),
///     user_agent: Some("CustomBot/1.0".to_string()),
///     timeout_secs: 60,
///     headless: true,
///     ..BrowserConfig::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct BrowserConfig {
    /// Optional proxy server URL.
    ///
    /// Format: `http://host:port` or `https://host:port`.
    /// When set, all browser traffic will be routed through this proxy.
    pub proxy: Option<String>,

    /// Optional custom user agent string.
    ///
    /// This will be sent with all browser requests. Use a descriptive name
    /// to identify your crawler to web servers.
    pub user_agent: Option<String>,

    /// Browser operation timeout in seconds.
    ///
    /// This applies to page navigation and content loading operations.
    /// If a page takes longer than this to load, the operation will fail.
    pub timeout_secs: u64,

    /// Whether to run the browser in headless mode.
    ///
    /// When `true`, the browser runs without a visible UI window, which is
    /// more efficient and suitable for server environments. Set to `false`
    /// for debugging to see the browser's actions visually.
    pub headless: bool,

    /// JS scripts to execute after page load (source, timeout_ms).
    pub post_load_js: Vec<(String, Option<u64>)>,

    /// Cookies to inject before first navigation.
    pub initial_cookies: Vec<crate::session::SessionCookie>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            proxy: None,
            user_agent: Some("Crawlery/1.0".to_string()),
            timeout_secs: 30,
            headless: true,
            post_load_js: vec![],
            initial_cookies: vec![],
        }
    }
}

/// Result of crawling a single URL using the browser.
///
/// Contains the raw HTML, cleaned text content, discovered links, and status information.
/// Unlike HTTP crawling, browser crawling may not always have a status code available.
#[derive(Debug, Clone)]
pub struct CrawlResult {
    /// The URL that was crawled.
    pub url: String,

    /// The raw HTML content after JavaScript execution.
    ///
    /// This is the fully-rendered HTML after all JavaScript has executed,
    /// not the initial server response.
    pub html: String,

    /// Cleaned, readable text extracted from the HTML.
    ///
    /// This has scripts, styles, navigation, and other non-content elements removed,
    /// making it suitable for RAG applications and content analysis.
    pub cleaned_content: String,

    /// List of absolute URLs discovered on the page.
    ///
    /// These are extracted from anchor tags after JavaScript execution,
    /// so dynamically-added links are included.
    pub links: Vec<String>,

    /// HTTP status code (may be None for browser crawling).
    ///
    /// Browser automation doesn't always expose the HTTP status code,
    /// so this field may be `None` even for successful requests.
    pub status_code: Option<u16>,
}

/// Browser-based web crawler using headless Chrome.
///
/// This crawler launches a headless Chrome browser instance and uses it to fetch
/// and render web pages. It's ideal for JavaScript-heavy sites but is slower and
/// more resource-intensive than HTTP crawling.
///
/// # Resource Management
///
/// The browser instance is automatically cleaned up when the `BrowserCrawler` is dropped.
/// Each `fetch()` call opens a new tab, which is closed when the method returns.
///
/// # Thread Safety
///
/// The underlying browser can be shared across threads using `Arc<BrowserCrawler>`.
pub struct BrowserCrawler {
    browser: Browser,
    config: BrowserConfig,
    collected_cookies: Arc<std::sync::Mutex<Vec<crate::session::SessionCookie>>>,
}

impl BrowserCrawler {
    /// Creates a new browser crawler with the given configuration.
    ///
    /// This launches a headless Chrome browser instance with the specified settings.
    /// The browser will remain running until the `BrowserCrawler` is dropped.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the browser
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the initialized crawler or an error if:
    /// - Chrome/Chromium is not installed or cannot be found
    /// - The browser fails to launch
    /// - The configuration is invalid
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawlery::browser::{BrowserCrawler, BrowserConfig};
    ///
    /// let config = BrowserConfig::default();
    /// let crawler = BrowserCrawler::new(config)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(config: BrowserConfig) -> Result<Self> {
        let mut launch_options = LaunchOptions::default_builder()
            .headless(config.headless)
            .build()
            .context("Failed to build launch options")?;

        if let Some(proxy) = &config.proxy {
            launch_options.proxy_server = Some(proxy.as_str());
        }

        let browser = Browser::new(launch_options).context("Failed to launch browser")?;
        Ok(Self {
            browser,
            config,
            collected_cookies: Arc::new(std::sync::Mutex::new(vec![])),
        })
    }

    /// Fetches and processes a URL using the browser, returning the crawl result.
    ///
    /// This method performs the following steps:
    /// 1. Opens a new browser tab
    /// 2. Sets the user agent (if configured)
    /// 3. Navigates to the URL
    /// 4. Waits for the page to fully load
    /// 5. Allows JavaScript to execute (2 second delay)
    /// 6. Extracts HTML, links, and cleaned content
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch
    ///
    /// # Returns
    ///
    /// Returns a `CrawlResult` containing the rendered HTML, cleaned text, and discovered links.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL is invalid
    /// - The browser tab cannot be created
    /// - Navigation fails (network error, DNS failure, etc.)
    /// - The page fails to load within the timeout period
    /// - JavaScript execution causes a critical error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crawlery::browser::{BrowserCrawler, BrowserConfig};
    /// let crawler = BrowserCrawler::new(BrowserConfig::default())?;
    /// let result = crawler.fetch("https://example.com")?;
    /// println!("Fetched {} links from {}", result.links.len(), result.url);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn fetch(&self, url: &str) -> Result<CrawlResult> {
        let tab = self.browser.new_tab().context("Failed to create new tab")?;

        if let Some(user_agent) = &self.config.user_agent {
            tab.set_user_agent(user_agent, None, None)
                .context("Failed to set user agent")?;
        }

        // Inject session cookies via CDP before navigation (works for any domain)
        if !self.config.initial_cookies.is_empty() {
            use headless_chrome::protocol::cdp::Network;
            let cookies: Vec<Network::CookieParam> = self
                .config
                .initial_cookies
                .iter()
                .map(|c| {
                    // Use explicit domain URL so CDP knows which domain to bind the cookie to
                    let url_str = c
                        .domain
                        .as_ref()
                        .map(|d| format!("https://{}", d.trim_start_matches('.')))
                        .unwrap_or_else(|| url.to_string());
                    Network::CookieParam {
                        name: c.name.clone(),
                        value: c.value.clone(),
                        url: Some(url_str),
                        domain: c.domain.clone(),
                        path: c.path.clone().or_else(|| Some("/".to_string())),
                        secure: None,
                        http_only: None,
                        same_site: None,
                        expires: None,
                        priority: None,
                        same_party: None,
                        source_scheme: None,
                        source_port: None,
                        partition_key: None,
                    }
                })
                .collect();
            if let Err(e) = tab.set_cookies(cookies) {
                eprintln!("[Browser] Warning: Failed to set session cookies: {}", e);
            }
        }

        eprintln!("[Browser] Navigating to: {}", url);
        tab.navigate_to(url).context("Failed to navigate to URL")?;
        tab.wait_until_navigated()
            .context("Failed to wait for navigation")?;

        eprintln!("[Browser] Page navigated, waiting for content to load...");

        // Wait for DOM to be ready and dynamic content to load
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Wait for document ready state
        let ready_script = "document.readyState";
        for attempt in 1..=10 {
            match tab.evaluate(ready_script, false) {
                Ok(result) => {
                    if let Some(state_value) = result.value {
                        if let Ok(state) = serde_json::from_value::<String>(state_value) {
                            eprintln!("[Browser] Document ready state: {}", state);
                            if state == "complete" {
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[Browser] Warning: Failed to check ready state: {}", e);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
            if attempt == 10 {
                eprintln!("[Browser] Warning: Document may not be fully loaded");
            }
        }

        // Additional wait for JavaScript-rendered content
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Run post_load_js hooks
        for (source, _timeout_ms) in &self.config.post_load_js {
            if let Err(e) = tab.evaluate(&format!("({})()", source), true) {
                eprintln!("[Browser] Warning: post_load JS failed: {}", e);
            }
        }

        // If hooks ran, allow any triggered navigation (e.g. form submission) to settle.
        // NOTE: do NOT call wait_until_navigated() here — it can race with Chrome background
        // navigations (update checks, safe browsing, etc.) and land on the wrong page.
        // A pure time-based settle + readyState poll is more reliable.
        if !self.config.post_load_js.is_empty() {
            std::thread::sleep(std::time::Duration::from_secs(4));
            for _ in 0..15 {
                match tab.evaluate("document.readyState", false) {
                    Ok(r) => {
                        if let Some(v) = r.value {
                            if serde_json::from_value::<String>(v)
                                .ok()
                                .as_deref()
                                == Some("complete")
                            {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        // Context may have been destroyed mid-navigation; wait more
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            eprintln!("[Browser] Post-hook page: {}", tab.get_url());
        }

        eprintln!("[Browser] Extracting content...");

        let html = tab.get_content().context("Failed to get page content")?;
        eprintln!("[Browser] HTML content length: {} bytes", html.len());

        // Use the tab's current URL (may differ from `url` after redirects/form submission)
        let actual_url = tab.get_url();
        let base_url = Url::parse(&actual_url)
            .or_else(|_| Url::parse(url))
            .context("Invalid URL")?;
        let links = self.extract_links(&html, &base_url)?;
        eprintln!("[Browser] Extracted {} links", links.len());

        let cleaned_content =
            content::md_readability(&html, &actual_url).unwrap_or_else(|_| Self::clean_content(&html));

        // Collect cookies via CDP (includes HttpOnly cookies invisible to document.cookie)
        if let Ok(cdp_cookies) = tab.get_cookies() {
            let mut collected = self.collected_cookies.lock().unwrap();
            for c in cdp_cookies {
                // Deduplicate by name+domain
                if !collected
                    .iter()
                    .any(|x| x.name == c.name && x.domain.as_deref() == Some(c.domain.as_str()))
                {
                    collected.push(crate::session::SessionCookie {
                        name: c.name,
                        value: c.value,
                        domain: Some(c.domain),
                        path: Some(c.path),
                    });
                }
            }
        }

        Ok(CrawlResult {
            url: url.to_string(),
            html,
            cleaned_content,
            links,
            status_code: None,
        })
    }

    /// Collect session data (cookies captured during crawl).
    pub fn collect_session(&self) -> crate::session::SessionData {
        let cookies = self.collected_cookies.lock().unwrap().clone();
        crate::session::SessionData {
            cookies,
            headers: std::collections::HashMap::new(),
            saved_at: None,
        }
    }

    /// Extracts and normalizes all links from the page using JavaScript.
    ///
    /// This method executes JavaScript in the browser to find all anchor tags and
    /// extract their `href` attributes. It filters for HTTP(S) URLs and deduplicates.
    ///
    /// # Arguments
    ///
    /// * `tab` - The browser tab to extract links from
    /// * `base_url` - The base URL (used for context, not currently for resolution)
    ///
    /// # Returns
    ///
    /// Returns a sorted, deduplicated vector of absolute URLs found on the page.
    /// Only HTTP and HTTPS links are included.
    ///
    /// # Errors
    ///
    /// Returns an error if JavaScript evaluation fails or the result cannot be parsed.
    fn extract_links(&self, html: &str, base_url: &Url) -> Result<Vec<String>> {
        let document = Html::parse_document(html);
        let selector = Selector::parse("a[href]").unwrap();
        let mut links: Vec<String> = document
            .select(&selector)
            .filter_map(|el| el.value().attr("href"))
            .filter_map(|href| base_url.join(href).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string())
            .collect();
        links.sort();
        links.dedup();
        Ok(links)
    }

    /// Cleans HTML content by removing non-content elements.
    ///
    /// This static method processes HTML to extract clean, readable text by:
    /// 1. Removing scripts, styles, navigation, headers, footers, and aside elements
    /// 2. Stripping all HTML tags
    /// 3. Decoding common HTML entities
    /// 4. Normalizing whitespace
    ///
    /// The result is suitable for RAG applications, search indexing, or content analysis.
    ///
    /// # Arguments
    ///
    /// * `html` - The HTML content to clean
    ///
    /// # Returns
    ///
    /// Returns a string containing cleaned text with normalized whitespace.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::browser::BrowserCrawler;
    ///
    /// let html = r#"
    ///     <html>
    ///         <script>console.log('test');</script>
    ///         <body><h1>Title</h1><p>Content here.</p></body>
    ///     </html>
    /// "#;
    ///
    /// let cleaned = BrowserCrawler::clean_content(html);
    /// assert!(cleaned.contains("Title"));
    /// assert!(cleaned.contains("Content here"));
    /// assert!(!cleaned.contains("console.log"));
    /// ```
    pub fn clean_content(html: &str) -> String {
        let mut cleaned = html.to_string();

        // Remove scripts, styles, and non-content elements
        let patterns = [
            r"(?s)<script[^>]*>.*?</script>",
            r"(?s)<style[^>]*>.*?</style>",
            r"(?s)<nav[^>]*>.*?</nav>",
            r"(?s)<header[^>]*>.*?</header>",
            r"(?s)<footer[^>]*>.*?</footer>",
            r"(?s)<aside[^>]*>.*?</aside>",
        ];

        for pattern in &patterns {
            let re = regex::Regex::new(pattern).unwrap();
            cleaned = re.replace_all(&cleaned, "").to_string();
        }

        // Remove HTML tags
        let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
        cleaned = tag_re.replace_all(&cleaned, " ").to_string();

        // Decode HTML entities
        cleaned = cleaned
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'");

        // Clean whitespace
        cleaned
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    // Extensibility hooks for future features

    /// Takes a screenshot of a page (not yet implemented).
    ///
    /// This is a placeholder for future functionality to capture screenshots
    /// of web pages during crawling.
    ///
    /// # Arguments
    ///
    /// * `_url` - The URL to screenshot
    /// * `_path` - The file path where the screenshot should be saved
    ///
    /// # Returns
    ///
    /// Currently returns an error indicating the feature is not implemented.
    ///
    /// # Future Implementation
    ///
    /// This method will navigate to the URL and save a PNG screenshot to the specified path.
    pub fn screenshot(&self, _url: &str, _path: &str) -> Result<()> {
        anyhow::bail!("Screenshot feature not yet implemented")
    }

    /// Executes custom JavaScript on a page (not yet implemented).
    ///
    /// This is a placeholder for future functionality to run custom JavaScript
    /// code on a page and return the results.
    ///
    /// # Arguments
    ///
    /// * `_url` - The URL to execute JavaScript on
    /// * `_script` - The JavaScript code to execute
    ///
    /// # Returns
    ///
    /// Currently returns an error indicating the feature is not implemented.
    ///
    /// # Future Implementation
    ///
    /// This method will navigate to the URL, execute the provided JavaScript,
    /// and return the result as a JSON value.
    pub fn execute_js(&self, _url: &str, _script: &str) -> Result<serde_json::Value> {
        anyhow::bail!("JavaScript execution feature not yet implemented")
    }

    /// Extracts cookies from a page (not yet implemented).
    ///
    /// This is a placeholder for future functionality to retrieve cookies
    /// from a page for session management or analysis.
    ///
    /// # Arguments
    ///
    /// * `_url` - The URL to extract cookies from
    ///
    /// # Returns
    ///
    /// Currently returns an error indicating the feature is not implemented.
    ///
    /// # Future Implementation
    ///
    /// This method will navigate to the URL and return all cookies as strings.
    pub fn get_cookies(&self, _url: &str) -> Result<Vec<String>> {
        let cookies = self.collected_cookies.lock().unwrap();
        Ok(cookies.iter().map(|c| format!("{}={}", c.name, c.value)).collect())
    }
}

impl Drop for BrowserCrawler {
    fn drop(&mut self) {
        // Browser cleanup handled by headless_chrome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_content() {
        let html = r#"
            <html>
                <head><style>body { color: red; }</style></head>
                <body>
                    <nav>Navigation</nav>
                    <main><h1>Title</h1><p>This is &nbsp; content.</p></main>
                    <script>console.log('test');</script>
                </body>
            </html>
        "#;

        let cleaned = BrowserCrawler::clean_content(html);
        assert!(cleaned.contains("Title"));
        assert!(cleaned.contains("content"));
        assert!(!cleaned.contains("<script"));
        assert!(!cleaned.contains("console.log"));
    }
}
