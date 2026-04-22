//! Crawlery - A flexible web crawler with HTTP and browser automation support.
//!
//! This library provides a robust web crawling framework that supports both HTTP-based
//! crawling and browser automation. It can be used as a library in your Rust projects
//! or through the command-line interface.
//!
//! # Examples
//!
//! ```no_run
//! use crawlery::{CrawlConfig, CrawlMode, Crawler};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = CrawlConfig::builder()
//!         .url("https://example.com")
//!         .mode(CrawlMode::Http)
//!         .max_depth(2)
//!         .build()?;
//!
//!     let crawler = Crawler::new(config);
//!     let results = crawler.crawl().await?;
//!
//!     println!("Crawled {} pages", results.len());
//!     Ok(())
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use url::Url;

pub mod browser;
pub mod content;
pub mod error;
pub mod http_client;
pub mod output;
pub mod state;

pub use error::{CrawlError, ErrorContext, Result};

// Re-export commonly used types
pub use reqwest;
pub use scraper;
pub use tokio;

/// The main crawler struct that orchestrates the crawling process.
///
/// This is the primary entry point for using the library. Create a crawler
/// with a configuration and call `crawl()` to start crawling.
#[derive(Debug)]
pub struct Crawler {
    config: CrawlConfig,
}

impl Crawler {
    /// Creates a new crawler with the given configuration.
    pub fn new(config: CrawlConfig) -> Self {
        Self { config }
    }

    /// Starts the crawling process and returns the results.
    ///
    /// This is an async function that will crawl the target URL and all discovered
    /// links up to the configured maximum depth.
    ///
    /// # Errors
    ///
    /// Returns an error if the crawl fails due to network issues, invalid URLs,
    /// or other errors during the crawling process.
    pub async fn crawl(&self) -> Result<Vec<CrawlResult>> {
        // Create or load crawl state
        let mut state = if let Some(state_file) = &self.config.state_file {
            if state_file.exists() {
                eprintln!("Resuming from saved state: {:?}", state_file);
                state::CrawlState::load(state_file)?
            } else {
                Self::create_state(&self.config)
            }
        } else {
            Self::create_state(&self.config)
        };

        // Choose crawler based on mode
        match self.config.mode {
            CrawlMode::Http => self.crawl_with_http(&mut state).await,
            CrawlMode::Browser => self.crawl_with_browser(&mut state).await,
        }
    }

    /// Create a new CrawlState from config
    fn create_state(config: &CrawlConfig) -> state::CrawlState {
        state::CrawlState::new(state::CrawlConfig {
            start_url: config.url.clone(),
            max_depth: config.max_depth,
            max_pages: config.max_pages,
            respect_robots_txt: config.respect_robots_txt,
        })
    }

    /// Crawl using HTTP client
    async fn crawl_with_http(&self, state: &mut state::CrawlState) -> Result<Vec<CrawlResult>> {
        // Create HTTP crawler
        let crawler = Arc::new(http_client::HttpCrawler::new(
            http_client::HttpCrawlerConfig {
                user_agent: self
                    .config
                    .user_agent
                    .clone()
                    .unwrap_or_else(|| "Crawlery/1.0 (RAG-optimized)".to_string()),
                delay_ms: self.config.delay_ms,
                timeout_secs: self.config.timeout_secs,
                proxies: self
                    .config
                    .proxy
                    .as_ref()
                    .map(|p| vec![p.url.clone()])
                    .unwrap_or_default(),
                respect_robots_txt: self.config.respect_robots_txt,
            },
        )?);

        self.crawl_loop(state, |url| {
            let url_owned = url.to_string();
            let crawler = crawler.clone();
            let extract = self.config.extract_content;
            async move {
                let result = crawler.fetch(&url_owned).await?;
                let content = if extract {
                    result.clean_text
                } else {
                    result.html.clone()
                };
                Ok((
                    result.status_code,
                    result.html.clone(),
                    content,
                    result.links,
                ))
            }
        })
        .await
    }

    /// Crawl using browser automation
    async fn crawl_with_browser(&self, state: &mut state::CrawlState) -> Result<Vec<CrawlResult>> {
        // Create browser crawler
        let crawler = Arc::new(browser::BrowserCrawler::new(browser::BrowserConfig {
            proxy: self.config.proxy.as_ref().map(|p| p.url.clone()),
            user_agent: self.config.user_agent.clone(),
            timeout_secs: self.config.timeout_secs,
            headless: true,
        })?);

        self.crawl_loop(state, |url| {
            let url_owned = url.to_string();
            let crawler = crawler.clone();
            let extract = self.config.extract_content;
            async move {
                let result = crawler.fetch(&url_owned)?;
                let content = if extract {
                    result.cleaned_content
                } else {
                    result.html.clone()
                };
                Ok((
                    result.status_code.unwrap_or(200),
                    result.html.clone(),
                    content,
                    result.links,
                ))
            }
        })
        .await
    }

    /// Main crawl loop - generic over HTTP and Browser crawlers
    async fn crawl_loop<F, Fut>(
        &self,
        state: &mut state::CrawlState,
        fetch_fn: F,
    ) -> Result<Vec<CrawlResult>>
    where
        F: Fn(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(u16, String, String, Vec<String>)>>,
    {
        let mut save_counter = 0;
        let mut error_count = 0;

        // Crawl loop
        while let Some((url, depth)) = state.next_pending() {
            // Skip if already visited
            if state.is_visited(&url) {
                continue;
            }

            // Check URL filters
            if !self.should_crawl_url(&url) {
                continue;
            }

            // Mark as visited
            state.mark_visited(url.clone());

            eprintln!("Crawling [depth={}]: {}", depth, url);

            // Fetch the page with retries
            let fetch_result = self.fetch_with_retry(&url, &fetch_fn).await;

            match fetch_result {
                Ok((status_code, html, clean_text, links)) => {
                    // Extract title from HTML
                    let title = Self::extract_title(&html);

                    // Create state result
                    let state_result = state::CrawlResult {
                        url: url.clone(),
                        depth,
                        status_code: Some(status_code),
                        title: title.clone(),
                        content: clean_text.clone(),
                        links: links.clone(),
                        timestamp: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };

                    // Filter and add discovered links to state
                    let filtered_links: Vec<String> = links
                        .into_iter()
                        .filter(|link| self.should_crawl_url(link))
                        .collect();

                    state.add_pending(filtered_links, depth);
                    state.add_result(state_result);

                    error_count = 0; // Reset error counter on success
                }
                Err(e) => {
                    eprintln!("Error fetching {}: {}", url, e);
                    error_count += 1;

                    // Add error result to state
                    state.add_result(state::CrawlResult {
                        url: url.clone(),
                        depth,
                        status_code: None,
                        title: None,
                        content: String::new(),
                        links: Vec::new(),
                        timestamp: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    });

                    // Stop if too many consecutive errors
                    if error_count > 10 {
                        eprintln!("Too many consecutive errors, stopping crawl");
                        break;
                    }
                }
            }

            // Save state periodically (every 10 pages)
            save_counter += 1;
            if save_counter % 10 == 0 {
                if let Some(state_file) = &self.config.state_file {
                    if let Err(e) = state.save(state_file) {
                        eprintln!("Warning: Failed to save state: {}", e);
                    }
                }
            }

            // Progress update
            if state.result_count().is_multiple_of(10) {
                eprintln!(
                    "Progress: {} pages crawled, {} pending",
                    state.result_count(),
                    state.pending_count()
                );
            }
        }

        // Final save
        if let Some(state_file) = &self.config.state_file {
            state.save(state_file)?;
        }

        eprintln!(
            "Crawl complete: {} pages in {} seconds",
            state.result_count(),
            state.elapsed_seconds()
        );

        // Convert state results to lib.rs CrawlResult format
        Ok(state
            .results()
            .iter()
            .map(Self::convert_state_result)
            .collect())
    }

    /// Fetch with retry logic
    async fn fetch_with_retry<F, Fut>(
        &self,
        url: &str,
        fetch_fn: &F,
    ) -> Result<(u16, String, String, Vec<String>)>
    where
        F: Fn(&str) -> Fut,
        Fut: std::future::Future<Output = Result<(u16, String, String, Vec<String>)>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match fetch_fn(url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        eprintln!(
                            "Retry {}/{} for {}",
                            attempt + 1,
                            self.config.max_retries,
                            url
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            self.config.delay_ms * (attempt as u64 + 1),
                        ))
                        .await;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Check if URL should be crawled based on include/exclude patterns
    fn should_crawl_url(&self, url: &str) -> bool {
        // Check include patterns
        if !self.config.include_patterns.is_empty() {
            let matches_include = self.config.include_patterns.iter().any(|pattern| {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(url))
                    .unwrap_or(false)
            });
            if !matches_include {
                return false;
            }
        }

        // Check exclude patterns
        if !self.config.exclude_patterns.is_empty() {
            let matches_exclude = self.config.exclude_patterns.iter().any(|pattern| {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(url))
                    .unwrap_or(false)
            });
            if matches_exclude {
                return false;
            }
        }

        true
    }

    /// Extract title from HTML
    fn extract_title(html: &str) -> Option<String> {
        use scraper::{Html, Selector};
        let document = Html::parse_document(html);
        let selector = Selector::parse("title").ok()?;
        document
            .select(&selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
    }

    /// Convert state::CrawlResult to lib.rs CrawlResult
    fn convert_state_result(state_result: &state::CrawlResult) -> CrawlResult {
        CrawlResult {
            url: state_result.url.clone(),
            status_code: state_result.status_code,
            title: state_result.title.clone(),
            content: state_result.content.clone(),
            links: state_result.links.clone(),
            metadata: HashMap::new(),
            timestamp: SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(state_result.timestamp),
            depth: state_result.depth,
            content_type: None,
            headers: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Returns a reference to the crawler's configuration.
    pub fn config(&self) -> &CrawlConfig {
        &self.config
    }
}

/// Configuration for the web crawler.
///
/// This struct contains all the settings needed to configure the crawler's behavior,
/// including the target URL, crawl mode, depth limits, and output settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    /// The starting URL to crawl
    pub url: String,

    /// The crawling mode (HTTP or Browser)
    pub mode: CrawlMode,

    /// Maximum depth to crawl (0 = only start URL)
    pub max_depth: usize,

    /// Maximum number of pages to crawl
    pub max_pages: Option<usize>,

    /// Optional path to save output
    pub output_path: Option<PathBuf>,

    /// Optional path to save/load crawl state for resumability
    pub state_file: Option<PathBuf>,

    /// Output format for results
    pub output_format: OutputFormat,

    /// Proxy settings
    pub proxy: Option<ProxyConfig>,

    /// User agent string
    pub user_agent: Option<String>,

    /// Request timeout in seconds
    pub timeout_secs: u64,

    /// Maximum number of concurrent requests
    pub max_concurrent_requests: usize,

    /// Delay between requests in milliseconds
    pub delay_ms: u64,

    /// Maximum number of retries per request
    pub max_retries: usize,

    /// Follow redirects
    pub follow_redirects: bool,

    /// Respect robots.txt
    pub respect_robots_txt: bool,

    /// URL patterns to include (regex)
    pub include_patterns: Vec<String>,

    /// URL patterns to exclude (regex)
    pub exclude_patterns: Vec<String>,

    /// Extract clean content using readability algorithm (for RAG/LLM applications)
    /// If false, returns raw HTML content. Default: false
    pub extract_content: bool,

    /// Additional HTTP headers
    pub headers: HashMap<String, String>,
}

impl CrawlConfig {
    /// Creates a new builder for constructing a CrawlConfig.
    pub fn builder() -> CrawlConfigBuilder {
        CrawlConfigBuilder::default()
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid (e.g., invalid URL format).
    pub fn validate(&self) -> Result<()> {
        // Validate URL
        Url::parse(&self.url).map_err(|e| CrawlError::InvalidUrl {
            url: self.url.clone(),
            reason: e.to_string(),
        })?;

        // Validate patterns
        for pattern in &self.include_patterns {
            regex::Regex::new(pattern).map_err(|e| CrawlError::ValidationError {
                field: "include_patterns".to_string(),
                message: format!("Invalid regex '{}': {}", pattern, e),
            })?;
        }

        for pattern in &self.exclude_patterns {
            regex::Regex::new(pattern).map_err(|e| CrawlError::ValidationError {
                field: "exclude_patterns".to_string(),
                message: format!("Invalid regex '{}': {}", pattern, e),
            })?;
        }

        Ok(())
    }

    /// Loads a configuration from a YAML recipe file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the YAML recipe file
    ///
    /// # Returns
    ///
    /// Returns the deserialized `CrawlConfig` from the file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read
    /// - The YAML is invalid or doesn't match the expected format
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawlery::CrawlConfig;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = CrawlConfig::from_file("recipe.yaml")?;
    /// println!("Loaded config for URL: {}", config.url);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| CrawlError::IoError {
            path: path.as_ref().to_string_lossy().to_string(),
            message: e.to_string(),
        })?;

        let config: CrawlConfig =
            serde_yaml::from_str(&content).map_err(|e| CrawlError::ValidationError {
                field: "recipe_file".to_string(),
                message: format!("Failed to parse YAML recipe file: {}", e),
            })?;

        // Validate the loaded configuration
        config.validate()?;

        Ok(config)
    }

    /// Saves the current configuration to a YAML recipe file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the recipe file should be written
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration cannot be serialized to YAML
    /// - The file cannot be created or written
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawlery::{CrawlConfig, CrawlMode};
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = CrawlConfig::builder()
    ///     .url("https://example.com")
    ///     .mode(CrawlMode::Http)
    ///     .max_depth(3)
    ///     .build()?;
    ///
    /// config.to_file("my_recipe.yaml")?;
    /// println!("Recipe saved successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yaml::to_string(self).map_err(|e| CrawlError::ValidationError {
            field: "config".to_string(),
            message: format!("Failed to serialize configuration to YAML: {}", e),
        })?;

        std::fs::write(path.as_ref(), yaml).map_err(|e| CrawlError::IoError {
            path: path.as_ref().to_string_lossy().to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}

/// Builder for CrawlConfig with sensible defaults.
#[derive(Debug, Default)]
pub struct CrawlConfigBuilder {
    url: Option<String>,
    mode: Option<CrawlMode>,
    max_depth: Option<usize>,
    max_pages: Option<usize>,
    output_path: Option<PathBuf>,
    state_file: Option<PathBuf>,
    output_format: Option<OutputFormat>,
    proxy: Option<ProxyConfig>,
    user_agent: Option<String>,
    timeout_secs: Option<u64>,
    max_concurrent_requests: Option<usize>,
    delay_ms: Option<u64>,
    max_retries: Option<usize>,
    follow_redirects: Option<bool>,
    respect_robots_txt: Option<bool>,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    extract_content: bool,
    headers: HashMap<String, String>,
}

impl CrawlConfigBuilder {
    /// Sets the starting URL to crawl.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the crawl mode.
    pub fn mode(mut self, mode: CrawlMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Sets the maximum crawl depth.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Sets the maximum number of pages to crawl.
    pub fn max_pages(mut self, pages: usize) -> Self {
        self.max_pages = Some(pages);
        self
    }

    /// Sets the output path.
    pub fn output_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_path = Some(path.into());
        self
    }

    /// Sets the state file path for resumable crawling.
    pub fn state_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_file = Some(path.into());
        self
    }

    /// Sets the output format.
    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = Some(format);
        self
    }

    /// Sets the proxy configuration.
    pub fn proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Sets the user agent string.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    /// Sets the request timeout in seconds.
    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Sets the maximum number of concurrent requests.
    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = Some(max);
        self
    }

    /// Sets the delay between requests in milliseconds.
    pub fn delay_ms(mut self, ms: u64) -> Self {
        self.delay_ms = Some(ms);
        self
    }

    /// Sets the maximum number of retries.
    pub fn max_retries(mut self, retries: usize) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Sets whether to follow redirects.
    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = Some(follow);
        self
    }

    /// Sets whether to respect robots.txt.
    pub fn respect_robots_txt(mut self, respect: bool) -> Self {
        self.respect_robots_txt = Some(respect);
        self
    }

    /// Adds a URL include pattern (regex).
    pub fn include_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Adds a URL exclude pattern (regex).
    pub fn exclude_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Enables content extraction using readability algorithm.
    /// When enabled, returns clean markdown-like content optimized for RAG/LLM.
    /// When disabled (default), returns raw HTML content.
    pub fn extract_content(mut self, extract: bool) -> Self {
        self.extract_content = extract;
        self
    }

    /// Adds an HTTP header.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Builds the CrawlConfig, validating all settings.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or invalid.
    pub fn build(self) -> Result<CrawlConfig> {
        let url = self.url.ok_or_else(|| CrawlError::ConfigError {
            message: "URL is required".to_string(),
        })?;

        let config = CrawlConfig {
            url,
            mode: self.mode.unwrap_or(CrawlMode::Http),
            max_depth: self.max_depth.unwrap_or(3),
            max_pages: self.max_pages,
            output_path: self.output_path,
            state_file: self.state_file,
            output_format: self.output_format.unwrap_or(OutputFormat::Json),
            proxy: self.proxy,
            user_agent: self.user_agent,
            timeout_secs: self.timeout_secs.unwrap_or(30),
            max_concurrent_requests: self.max_concurrent_requests.unwrap_or(10),
            delay_ms: self.delay_ms.unwrap_or(0),
            max_retries: self.max_retries.unwrap_or(3),
            follow_redirects: self.follow_redirects.unwrap_or(true),
            respect_robots_txt: self.respect_robots_txt.unwrap_or(true),
            include_patterns: self.include_patterns,
            exclude_patterns: self.exclude_patterns,
            extract_content: self.extract_content,
            headers: self.headers,
        };

        config.validate()?;
        Ok(config)
    }
}

/// Crawling mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrawlMode {
    /// Use HTTP client for fast, lightweight crawling
    Http,

    /// Use headless browser for JavaScript-heavy sites
    Browser,
}

impl std::fmt::Display for CrawlMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrawlMode::Http => write!(f, "http"),
            CrawlMode::Browser => write!(f, "browser"),
        }
    }
}

impl std::str::FromStr for CrawlMode {
    type Err = CrawlError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "http" => Ok(CrawlMode::Http),
            "browser" => Ok(CrawlMode::Browser),
            _ => Err(CrawlError::ValidationError {
                field: "mode".to_string(),
                message: format!("Invalid mode '{}'. Valid options: http, browser", s),
            }),
        }
    }
}

/// Result of crawling a single URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    /// The URL that was crawled
    pub url: String,

    /// HTTP status code
    pub status_code: Option<u16>,

    /// The page title
    pub title: Option<String>,

    /// The page content (HTML or text)
    pub content: String,

    /// Links discovered on the page
    pub links: Vec<String>,

    /// Metadata extracted from the page
    pub metadata: HashMap<String, String>,

    /// Timestamp when the page was crawled
    pub timestamp: SystemTime,

    /// Depth level in the crawl tree
    pub depth: usize,

    /// Content type of the response
    pub content_type: Option<String>,

    /// Response headers
    pub headers: HashMap<String, String>,

    /// Any errors that occurred (non-fatal)
    pub errors: Vec<String>,
}

impl CrawlResult {
    /// Creates a new CrawlResult with the minimum required fields.
    pub fn new(url: String, content: String, depth: usize) -> Self {
        Self {
            url,
            status_code: None,
            title: None,
            content,
            links: Vec::new(),
            metadata: HashMap::new(),
            timestamp: SystemTime::now(),
            depth,
            content_type: None,
            headers: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Returns the number of links found on this page.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    /// Checks if the crawl was successful (2xx status code).
    pub fn is_success(&self) -> bool {
        self.status_code
            .is_some_and(|code| (200..300).contains(&code))
    }
}

/// Output format for crawl results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// JSON format
    Json,

    /// JSON with pretty printing
    #[serde(alias = "json-pretty")]
    JsonPretty,

    /// Markdown format
    Markdown,

    /// CSV format
    Csv,

    /// Plain text
    Text,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::JsonPretty => write!(f, "json-pretty"),
            OutputFormat::Markdown => write!(f, "markdown"),
            OutputFormat::Csv => write!(f, "csv"),
            OutputFormat::Text => write!(f, "text"),
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = CrawlError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "json-pretty" => Ok(OutputFormat::JsonPretty),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "csv" => Ok(OutputFormat::Csv),
            "text" | "txt" => Ok(OutputFormat::Text),
            _ => Err(CrawlError::ValidationError {
                field: "output_format".to_string(),
                message: format!(
                    "Invalid format '{}'. Valid options: json, json-pretty, markdown, csv, text",
                    s
                ),
            }),
        }
    }
}

/// Proxy configuration for the crawler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Proxy URL (e.g., "<http://proxy.example.com:8080>")
    pub url: String,

    /// Optional username for proxy authentication
    pub username: Option<String>,

    /// Optional password for proxy authentication
    pub password: Option<String>,
}

impl ProxyConfig {
    /// Creates a new proxy configuration.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            username: None,
            password: None,
        }
    }

    /// Sets the authentication credentials.
    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crawl_config_builder() {
        let config = CrawlConfig::builder()
            .url("https://example.com")
            .mode(CrawlMode::Http)
            .max_depth(2)
            .build()
            .unwrap();

        assert_eq!(config.url, "https://example.com");
        assert_eq!(config.mode, CrawlMode::Http);
        assert_eq!(config.max_depth, 2);
    }

    #[test]
    fn test_crawl_config_builder_missing_url() {
        let result = CrawlConfig::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_crawl_mode_from_str() {
        assert_eq!("http".parse::<CrawlMode>().unwrap(), CrawlMode::Http);
        assert_eq!("browser".parse::<CrawlMode>().unwrap(), CrawlMode::Browser);
        assert!("invalid".parse::<CrawlMode>().is_err());
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "markdown".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert_eq!(
            "md".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_crawl_result_creation() {
        let result = CrawlResult::new(
            "https://example.com".to_string(),
            "<html></html>".to_string(),
            0,
        );
        assert_eq!(result.url, "https://example.com");
        assert_eq!(result.depth, 0);
        assert_eq!(result.link_count(), 0);
    }

    #[test]
    fn test_crawl_result_is_success() {
        let mut result =
            CrawlResult::new("https://example.com".to_string(), "content".to_string(), 0);

        result.status_code = Some(200);
        assert!(result.is_success());

        result.status_code = Some(404);
        assert!(!result.is_success());

        result.status_code = None;
        assert!(!result.is_success());
    }

    #[test]
    fn test_proxy_config() {
        let proxy = ProxyConfig::new("http://proxy.example.com:8080").with_auth("user", "pass");

        assert_eq!(proxy.url, "http://proxy.example.com:8080");
        assert_eq!(proxy.username, Some("user".to_string()));
        assert_eq!(proxy.password, Some("pass".to_string()));
    }

    #[test]
    fn test_config_to_file() {
        use std::fs;
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        let config = CrawlConfig::builder()
            .url("https://example.com")
            .mode(CrawlMode::Http)
            .max_depth(3)
            .max_pages(100)
            .build()
            .unwrap();

        // Save config to file
        config.to_file(file_path).unwrap();

        // Verify file exists and contains YAML
        let content = fs::read_to_string(file_path).unwrap();
        assert!(content.contains("url:"));
        assert!(content.contains("https://example.com"));
        assert!(content.contains("mode: http"));
        assert!(content.contains("max_depth: 3"));
    }

    #[test]
    fn test_config_from_file() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        // Create and save a config
        let original_config = CrawlConfig::builder()
            .url("https://test.example.com")
            .mode(CrawlMode::Browser)
            .max_depth(5)
            .max_pages(200)
            .output_format(OutputFormat::Markdown)
            .build()
            .unwrap();

        original_config.to_file(file_path).unwrap();

        // Load config from file
        let loaded_config = CrawlConfig::from_file(file_path).unwrap();

        // Verify loaded config matches original
        assert_eq!(loaded_config.url, "https://test.example.com");
        assert_eq!(loaded_config.mode, CrawlMode::Browser);
        assert_eq!(loaded_config.max_depth, 5);
        assert_eq!(loaded_config.max_pages, Some(200));
        assert_eq!(loaded_config.output_format, OutputFormat::Markdown);
    }

    #[test]
    fn test_config_roundtrip_with_all_fields() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        // Create config with many fields
        let original_config = CrawlConfig::builder()
            .url("https://docs.example.com")
            .mode(CrawlMode::Http)
            .max_depth(4)
            .max_pages(500)
            .output_path(PathBuf::from("output.json"))
            .state_file(PathBuf::from("state.json"))
            .output_format(OutputFormat::JsonPretty)
            .timeout_secs(60)
            .max_concurrent_requests(5)
            .delay_ms(500)
            .max_retries(3)
            .follow_redirects(true)
            .respect_robots_txt(true)
            .include_pattern(r"^https://docs\.example\.com/.*")
            .exclude_pattern(r"\.pdf$")
            .exclude_pattern(r"/login")
            .extract_content(true)
            .header("User-Agent".to_string(), "TestBot/1.0".to_string())
            .build()
            .unwrap();

        // Save and load
        original_config.to_file(file_path).unwrap();
        let loaded_config = CrawlConfig::from_file(file_path).unwrap();

        // Verify all fields
        assert_eq!(loaded_config.url, original_config.url);
        assert_eq!(loaded_config.mode, original_config.mode);
        assert_eq!(loaded_config.max_depth, original_config.max_depth);
        assert_eq!(loaded_config.max_pages, original_config.max_pages);
        assert_eq!(loaded_config.timeout_secs, original_config.timeout_secs);
        assert_eq!(
            loaded_config.max_concurrent_requests,
            original_config.max_concurrent_requests
        );
        assert_eq!(loaded_config.delay_ms, original_config.delay_ms);
        assert_eq!(loaded_config.max_retries, original_config.max_retries);
        assert_eq!(
            loaded_config.follow_redirects,
            original_config.follow_redirects
        );
        assert_eq!(
            loaded_config.respect_robots_txt,
            original_config.respect_robots_txt
        );
        assert_eq!(
            loaded_config.include_patterns,
            original_config.include_patterns
        );
        assert_eq!(
            loaded_config.exclude_patterns,
            original_config.exclude_patterns
        );
        assert_eq!(
            loaded_config.extract_content,
            original_config.extract_content
        );
        assert_eq!(loaded_config.headers, original_config.headers);
    }

    #[test]
    fn test_config_from_file_invalid_yaml() {
        use std::fs;
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        // Write invalid YAML
        fs::write(file_path, "invalid: yaml: content: {{{").unwrap();

        // Should fail to load
        let result = CrawlConfig::from_file(file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_from_file_invalid_url() {
        use std::fs;
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        // Write YAML with invalid URL
        let yaml = r#"
url: "not a valid url"
mode: http
max_depth: 3
output_format: json
timeout_secs: 30
max_concurrent_requests: 10
delay_ms: 0
max_retries: 3
follow_redirects: true
respect_robots_txt: true
include_patterns: []
exclude_patterns: []
extract_content: false
headers: {}
"#;
        fs::write(file_path, yaml).unwrap();

        // Should fail validation
        let result = CrawlConfig::from_file(file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_patterns() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path();

        let original_config = CrawlConfig::builder()
            .url("https://example.com")
            .include_pattern(r"^https://example\.com/docs/.*")
            .include_pattern(r"^https://example\.com/api/.*")
            .exclude_pattern(r"\.pdf$")
            .exclude_pattern(r"\.zip$")
            .exclude_pattern(r"/login")
            .build()
            .unwrap();

        original_config.to_file(file_path).unwrap();
        let loaded_config = CrawlConfig::from_file(file_path).unwrap();

        assert_eq!(loaded_config.include_patterns.len(), 2);
        assert_eq!(loaded_config.exclude_patterns.len(), 3);
        assert_eq!(
            loaded_config.include_patterns[0],
            r"^https://example\.com/docs/.*"
        );
        assert_eq!(loaded_config.exclude_patterns[0], r"\.pdf$");
    }

    #[test]
    fn test_config_from_file_nonexistent() {
        let result = CrawlConfig::from_file("nonexistent_file.yaml");
        assert!(result.is_err());
    }
}
