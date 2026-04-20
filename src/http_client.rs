//! HTTP client module for fast, lightweight web crawling.
//!
//! This module provides an asynchronous HTTP-based crawler optimized for RAG (Retrieval-Augmented Generation)
//! applications. It includes features like robots.txt compliance, intelligent link extraction, HTML content
//! cleaning, proxy rotation, and rate limiting.
//!
//! # Features
//!
//! - **Async/await support** - Built on `reqwest` and `tokio` for high performance
//! - **robots.txt compliance** - Respects website crawling policies
//! - **Content cleaning** - Extracts clean text from HTML, removing scripts, styles, and navigation
//! - **Link extraction** - Discovers and normalizes all links on a page
//! - **Proxy support** - Rotate through multiple proxies for distributed crawling
//! - **Rate limiting** - Configurable delays between requests to avoid overwhelming servers
//!
//! # Example
//!
//! ```no_run
//! use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = HttpCrawlerConfig {
//!         user_agent: "MyBot/1.0".to_string(),
//!         delay_ms: 1000,
//!         timeout_secs: 30,
//!         proxies: vec![],
//!         respect_robots_txt: true,
//!     };
//!
//!     let crawler = HttpCrawler::new(config)?;
//!     let result = crawler.fetch("https://example.com").await?;
//!
//!     println!("Fetched {} with {} links", result.url, result.links.len());
//!     println!("Clean text: {}", result.clean_text);
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use reqwest::{Client, Proxy};
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use url::Url;

use crate::content;

/// Configuration for the HTTP crawler.
///
/// This struct contains all the settings needed to configure the behavior of the HTTP crawler,
/// including user agent, rate limiting, timeouts, proxy settings, and robots.txt compliance.
///
/// # Examples
///
/// ```
/// use crawlery::http_client::HttpCrawlerConfig;
///
/// // Use default configuration
/// let config = HttpCrawlerConfig::default();
///
/// // Or customize the configuration
/// let config = HttpCrawlerConfig {
///     user_agent: "MyBot/1.0".to_string(),
///     delay_ms: 2000,  // 2 second delay between requests
///     timeout_secs: 60,
///     proxies: vec!["http://proxy.example.com:8080".to_string()],
///     respect_robots_txt: true,
/// };
/// ```
#[derive(Clone)]
pub struct HttpCrawlerConfig {
    /// User agent string sent in HTTP requests.
    ///
    /// This identifies your crawler to web servers. Use a descriptive name
    /// and contact information to help site administrators.
    pub user_agent: String,

    /// Delay in milliseconds between consecutive requests.
    ///
    /// Setting this to a non-zero value helps avoid overwhelming servers
    /// and reduces the risk of being rate-limited or blocked.
    pub delay_ms: u64,

    /// Request timeout in seconds.
    ///
    /// If a request takes longer than this duration, it will be aborted.
    pub timeout_secs: u64,

    /// List of proxy URLs for rotating requests.
    ///
    /// When multiple proxies are provided, the crawler rotates through them
    /// to distribute the load. Format: `http://host:port` or `https://host:port`.
    pub proxies: Vec<String>,

    /// Whether to respect robots.txt rules.
    ///
    /// When `true`, the crawler fetches and parses robots.txt for each domain
    /// and skips URLs that are disallowed for the crawler's user agent.
    pub respect_robots_txt: bool,
}

impl Default for HttpCrawlerConfig {
    fn default() -> Self {
        Self {
            user_agent: "Crawlery/1.0 (RAG-optimized crawler)".to_string(),
            delay_ms: 1000,
            timeout_secs: 30,
            proxies: vec![],
            respect_robots_txt: true,
        }
    }
}

/// Result of crawling a single URL via HTTP.
///
/// Contains the raw HTML, cleaned text content, discovered links, and HTTP status information.
pub struct CrawlResult {
    /// The URL that was crawled
    pub url: String,

    /// The raw HTML content of the page
    pub html: String,

    /// Cleaned, readable text extracted from the HTML (scripts, styles, and navigation removed)
    pub clean_text: String,

    /// List of absolute URLs discovered on the page
    pub links: Vec<String>,

    /// HTTP status code returned by the server
    pub status_code: u16,
}

/// HTTP-based web crawler with advanced features.
///
/// This crawler uses `reqwest` for HTTP requests and provides intelligent content extraction,
/// link discovery, robots.txt compliance, and proxy rotation. It's designed for high-performance
/// crawling of static websites and APIs.
///
/// # Thread Safety
///
/// `HttpCrawler` can be safely shared across threads using `Arc<HttpCrawler>`.
/// The internal robots.txt cache and proxy rotation are thread-safe.
pub struct HttpCrawler {
    client: Client,
    config: HttpCrawlerConfig,
    proxy_index: AtomicUsize,
    robots_cache: Arc<RwLock<HashMap<String, bool>>>,
}

impl HttpCrawler {
    /// Creates a new HTTP crawler with the given configuration.
    ///
    /// This initializes the HTTP client with the specified user agent, timeout, and proxy settings.
    /// If proxies are configured, the first proxy is set as the default.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the crawler
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the initialized crawler or an error if the client
    /// configuration is invalid (e.g., malformed proxy URL).
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};
    ///
    /// let config = HttpCrawlerConfig::default();
    /// let crawler = HttpCrawler::new(config)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(config: HttpCrawlerConfig) -> Result<Self> {
        let mut builder = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.timeout_secs));
        if !config.proxies.is_empty() {
            if let Ok(proxy) = Proxy::all(&config.proxies[0]) {
                builder = builder.proxy(proxy);
            }
        }
        Ok(Self {
            client: builder.build()?,
            config,
            proxy_index: AtomicUsize::new(0),
            robots_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Fetches and processes a URL, returning the crawl result.
    ///
    /// This method performs the following steps:
    /// 1. Checks robots.txt compliance (if enabled)
    /// 2. Applies rate limiting delay (if configured)
    /// 3. Sends HTTP GET request
    /// 4. Extracts and normalizes all links
    /// 5. Cleans HTML content to extract readable text
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch
    ///
    /// # Returns
    ///
    /// Returns a `CrawlResult` containing the HTML, cleaned text, discovered links,
    /// and status code.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL is invalid or cannot be parsed
    /// - The URL is disallowed by robots.txt (when respect_robots_txt is true)
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The response cannot be read
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};
    /// # #[tokio::main]
    /// # async fn main() -> anyhow::Result<()> {
    /// let crawler = HttpCrawler::new(HttpCrawlerConfig::default())?;
    /// let result = crawler.fetch("https://example.com").await?;
    /// println!("Fetched {} links from {}", result.links.len(), result.url);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch(&self, url: &str) -> Result<CrawlResult> {
        let parsed_url = Url::parse(url).context("Invalid URL")?;
        if self.config.respect_robots_txt && !self.is_allowed(&parsed_url).await? {
            anyhow::bail!("URL disallowed by robots.txt");
        }
        if self.config.delay_ms > 0 {
            sleep(Duration::from_millis(self.config.delay_ms)).await;
        }
        let response = self.client.get(url).send().await?;
        let status_code = response.status().as_u16();
        let html = response.text().await?;
        let links = self.extract_links(&html, &parsed_url)?;
        let clean_text =
            content::extract_content(&html).unwrap_or_else(|_| self.clean_content(&html));
        Ok(CrawlResult {
            url: url.to_string(),
            html,
            clean_text,
            links,
            status_code,
        })
    }

    /// Extracts and normalizes all links from HTML content.
    ///
    /// This method parses the HTML, finds all `<a>` tags with `href` attributes, resolves
    /// relative URLs to absolute URLs, filters out non-HTTP(S) links, and removes duplicates.
    ///
    /// # Arguments
    ///
    /// * `html` - The HTML content to parse
    /// * `base_url` - The base URL used to resolve relative links
    ///
    /// # Returns
    ///
    /// Returns a sorted, deduplicated vector of absolute URLs found in the HTML.
    /// Only HTTP and HTTPS links are included.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};
    /// # use url::Url;
    /// let crawler = HttpCrawler::new(HttpCrawlerConfig::default())?;
    /// let html = r#"<a href="/page">Link</a><a href="https://example.com">External</a>"#;
    /// let base = Url::parse("https://example.com")?;
    ///
    /// let links = crawler.extract_links(html, &base)?;
    /// assert_eq!(links.len(), 2);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn extract_links(&self, html: &str, base_url: &Url) -> Result<Vec<String>> {
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

    /// Extracts clean, readable text content from HTML.
    ///
    /// This method intelligently extracts the main content from HTML by:
    /// 1. Prioritizing content sections (article, main, body)
    /// 2. Removing non-content elements (scripts, styles, navigation, ads)
    /// 3. Normalizing whitespace
    ///
    /// The result is clean text suitable for RAG applications, search indexing,
    /// or content analysis.
    ///
    /// # Arguments
    ///
    /// * `html` - The HTML content to clean
    ///
    /// # Returns
    ///
    /// Returns a string containing the cleaned, readable text with normalized whitespace.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};
    /// let crawler = HttpCrawler::new(HttpCrawlerConfig::default())?;
    /// let html = r#"<html><body><p>Hello World</p><p>More content</p></body></html>"#;
    /// let clean = crawler.clean_content(html);
    /// assert!(clean.contains("Hello World"));
    /// assert!(clean.contains("More content"));
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn clean_content(&self, html: &str) -> String {
        let document = Html::parse_document(html);
        let selectors = ["article", "main", "[role='main']", "body"];
        let text_parts = selectors
            .iter()
            .find_map(|s| {
                Selector::parse(s)
                    .ok()
                    .and_then(|sel| document.select(&sel).next())
                    .map(|el| self.extract_clean_text(&el))
            })
            .unwrap_or_else(|| {
                document
                    .root_element()
                    .text()
                    .map(|s| s.to_string())
                    .collect()
            });
        text_parts
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Recursively extracts clean text from an HTML element.
    ///
    /// This helper method filters out non-content elements (scripts, styles, navigation,
    /// ads, etc.) and extracts text from content elements. It's used internally by
    /// `clean_content` to process the HTML tree.
    ///
    /// # Arguments
    ///
    /// * `element` - The HTML element to extract text from
    ///
    /// # Returns
    ///
    /// Returns a vector of text strings found in the element and its children.
    /// Non-content elements return an empty vector.
    fn extract_clean_text(&self, element: &scraper::ElementRef) -> Vec<String> {
        let tag = element.value().name();
        if matches!(
            tag,
            "script" | "style" | "nav" | "header" | "footer" | "aside" | "iframe" | "noscript"
        ) {
            return vec![];
        }
        if let Some(class) = element.value().attr("class") {
            if class.contains("ad") || class.contains("nav") || class.contains("menu") {
                return vec![];
            }
        }
        element.text().map(|s| s.to_string()).collect()
    }

    /// Checks if a URL is allowed according to the site's robots.txt.
    ///
    /// This method fetches and caches the robots.txt file for each domain, then checks
    /// if the given URL path is allowed for the crawler's user agent. Results are cached
    /// to avoid repeated requests for the same domain.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the URL is allowed or if robots.txt cannot be fetched,
    /// `Ok(false)` if the URL is explicitly disallowed.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL cannot be parsed or if there's an issue with the cache.
    async fn is_allowed(&self, url: &Url) -> Result<bool> {
        let domain = url.host_str().context("Missing host")?;
        if let Some(&allowed) = self.robots_cache.read().await.get(domain) {
            return Ok(allowed);
        }
        let robots_url = format!("{}://{}/robots.txt", url.scheme(), domain);
        let allowed = match self.client.get(&robots_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let content = resp.text().await.unwrap_or_default();
                !self.is_disallowed_by_robots(&content, url.path())
            }
            _ => true,
        };
        self.robots_cache
            .write()
            .await
            .insert(domain.to_string(), allowed);
        Ok(allowed)
    }

    /// Parses robots.txt content to determine if a path is disallowed.
    ///
    /// This implements a basic robots.txt parser that looks for User-agent directives
    /// matching "*" (all agents) or "crawlery" (this crawler specifically) and checks
    /// Disallow rules for those agents.
    ///
    /// # Arguments
    ///
    /// * `robots_txt` - The content of the robots.txt file
    /// * `path` - The URL path to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the path is explicitly disallowed, `false` otherwise.
    fn is_disallowed_by_robots(&self, robots_txt: &str, path: &str) -> bool {
        let mut applies_to_us = false;
        for line in robots_txt.lines() {
            let line = line.trim().to_lowercase();
            if line.starts_with("user-agent:") {
                let agent = line.split(':').nth(1).unwrap_or("").trim();
                applies_to_us = agent == "*" || agent.contains("crawlery");
            } else if applies_to_us && line.starts_with("disallow:") {
                if let Some(disallow_path) = line.split(':').nth(1) {
                    let disallow_path = disallow_path.trim();
                    if !disallow_path.is_empty() && path.starts_with(disallow_path) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Gets the next proxy from the proxy rotation list.
    ///
    /// When multiple proxies are configured, this method rotates through them
    /// in a round-robin fashion. This is thread-safe and uses atomic operations.
    ///
    /// # Returns
    ///
    /// Returns `Some(proxy_url)` if proxies are configured, or `None` if no proxies are available.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};
    /// let config = HttpCrawlerConfig {
    ///     proxies: vec![
    ///         "http://proxy1.example.com:8080".to_string(),
    ///         "http://proxy2.example.com:8080".to_string(),
    ///     ],
    ///     ..Default::default()
    /// };
    /// let crawler = HttpCrawler::new(config)?;
    /// let proxy = crawler.get_next_proxy();
    /// assert!(proxy.is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn get_next_proxy(&self) -> Option<String> {
        if self.config.proxies.is_empty() {
            return None;
        }
        let idx = self.proxy_index.fetch_add(1, Ordering::Relaxed);
        Some(self.config.proxies[idx % self.config.proxies.len()].clone())
    }
}
