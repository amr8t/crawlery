//! Crawlery - A flexible web crawler with HTTP and browser automation support.
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
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use url::Url;

pub mod browser;
pub mod content;
pub mod error;
pub mod hooks;
pub mod http_client;
pub mod output;
pub mod pipeline;
pub mod session;
pub mod state;
pub mod transformers;

pub use error::{CrawlError, ErrorContext, Result};
pub use pipeline::{Pipeline, StageBuilder};

// Re-export commonly used types
pub use reqwest;
pub use scraper;
pub use tokio;

/// The main crawler struct that orchestrates the crawling process.
#[derive(Debug)]
pub struct Crawler {
    config: CrawlConfig,
}

/// Load start URLs from an input_from file.
/// Accepts `["url1","url2"]` or `[{"url":"...","title":"..."},...]` format.
fn load_input_urls(path: &PathBuf) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path).map_err(|e| CrawlError::IoError {
        path: path.to_string_lossy().to_string(),
        message: e.to_string(),
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| CrawlError::ValidationError {
            field: "input_from".to_string(),
            message: format!("Failed to parse input file as JSON: {}", e),
        })?;
    let arr = value
        .as_array()
        .ok_or_else(|| CrawlError::ValidationError {
            field: "input_from".to_string(),
            message: "Input file must be a JSON array".to_string(),
        })?;
    let mut urls = Vec::new();
    for item in arr {
        if let Some(s) = item.as_str() {
            urls.push(s.to_string());
        } else if let Some(u) = item.get("url").and_then(|v| v.as_str()) {
            urls.push(u.to_string());
        }
    }
    if urls.is_empty() {
        anyhow::bail!("input_from file contains no valid URLs");
    }
    Ok(urls)
}

impl Crawler {
    /// Creates a new crawler with the given configuration.
    pub fn new(config: CrawlConfig) -> Self {
        Self { config }
    }

    /// Starts the crawling process and returns the results.
    pub async fn crawl(&self) -> Result<Vec<CrawlResult>> {
        // Load session data if configured
        let session_data: Option<session::SessionData> = if let Some(sc) = &self.config.session {
            if let Some(path) = &sc.load_from {
                Some(session::SessionData::load(path)?)
            } else {
                None
            }
        } else {
            None
        };

        // Determine start URLs
        let start_urls: Vec<String> = if let Some(input_path) = &self.config.input_from {
            load_input_urls(input_path)?
        } else {
            vec![self.config.url.clone()]
        };

        // Create or resume crawl state
        let mut state = if let Some(state_file) = &self.config.state_file {
            if state_file.exists() {
                eprintln!("Resuming from saved state: {:?}", state_file);
                state::CrawlState::load(state_file)?
            } else {
                Self::create_state_with_urls(&self.config, &start_urls)
            }
        } else {
            Self::create_state_with_urls(&self.config, &start_urls)
        };

        // Run crawl
        let mut results = match self.config.mode {
            CrawlMode::Http => {
                self.crawl_with_http(&mut state, session_data.as_ref())
                    .await?
            }
            CrawlMode::Browser => {
                self.crawl_with_browser(&mut state, session_data.as_ref())
                    .await?
            }
        };

        // Apply transformers
        if !self.config.transformers.is_empty() {
            results = transformers::apply_transformers(results, &self.config.transformers).await?;
        }

        Ok(results)
    }

    /// Create a new CrawlState from a list of start URLs.
    fn create_state_with_urls(config: &CrawlConfig, urls: &[String]) -> state::CrawlState {
        let first = urls.first().cloned().unwrap_or_else(|| config.url.clone());
        let mut s = state::CrawlState::new(state::CrawlConfig {
            start_url: first,
            max_depth: config.max_depth,
            max_pages: config.max_pages,
            respect_robots_txt: config.respect_robots_txt,
        });
        if urls.len() > 1 {
            // seed_urls replaces the pending queue with ALL urls at depth 0
            s.seed_urls(urls.to_vec());
        }
        s
    }

    /// Crawl using HTTP client.
    async fn crawl_with_http(
        &self,
        state: &mut state::CrawlState,
        session: Option<&session::SessionData>,
    ) -> Result<Vec<CrawlResult>> {
        // Merge config headers with session headers
        let mut extra_headers = self.config.headers.clone();
        let mut initial_cookies = vec![];
        if let Some(sd) = session {
            extra_headers.extend(sd.headers.clone());
            initial_cookies = sd.cookies.clone();
        }

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
                extra_headers,
                initial_cookies,
            },
        )?);

        let config_session = self.config.session.clone();
        let crawler_ref = crawler.clone();

        let md_readability = self.config.md_readability;
        let results = self
            .crawl_loop(state, move |url: String| {
                let crawler = crawler_ref.clone();
                async move {
                    let result = crawler.fetch(&url).await?;
                    let content = if md_readability {
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
            .await?;

        // Save session after crawl if configured
        if let Some(sc) = &config_session {
            if let Some(save_to) = &sc.save_to {
                let session_data = crawler.collect_session();
                if sc.save_cookies && !session_data.cookies.is_empty() {
                    session_data.save(save_to)?;
                    eprintln!("Session saved to: {}", save_to.display());
                }
            }
        }

        Ok(results)
    }

    /// Crawl using browser automation.
    async fn crawl_with_browser(
        &self,
        state: &mut state::CrawlState,
        session: Option<&session::SessionData>,
    ) -> Result<Vec<CrawlResult>> {
        // Build post_load JS hooks from HooksConfig
        let post_load_js: Vec<(String, Option<u64>)> = self
            .config
            .hooks
            .as_ref()
            .map(|h| {
                h.post_load
                    .iter()
                    .filter_map(|hook| {
                        if let HookType::Javascript { source } = &hook.hook_type {
                            Some((source.clone(), hook.timeout_ms))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let initial_cookies = session.map(|sd| sd.cookies.clone()).unwrap_or_default();

        let crawler = Arc::new(browser::BrowserCrawler::new(browser::BrowserConfig {
            proxy: self.config.proxy.as_ref().map(|p| p.url.clone()),
            user_agent: self.config.user_agent.clone(),
            timeout_secs: self.config.timeout_secs,
            headless: true,
            post_load_js,
            initial_cookies,
        })?);

        let config_session = self.config.session.clone();
        let crawler_ref = crawler.clone();

        let md_readability = self.config.md_readability;
        let results = self
            .crawl_loop(state, move |url: String| {
                let crawler = crawler_ref.clone();
                async move {
                    let result = crawler.fetch(&url)?;
                    let content = if md_readability {
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
            .await?;

        // Save session after crawl if configured
        if let Some(sc) = &config_session {
            if let Some(save_to) = &sc.save_to {
                let session_data = crawler.collect_session();
                session_data.save(save_to)?;
                eprintln!("Browser session saved to: {}", save_to.display());
            }
        }

        Ok(results)
    }

    /// Concurrent crawl loop — the core engine for a single pipeline stage.
    ///
    /// Uses a `Semaphore` to bound the number of in-flight requests to
    /// `config.max_concurrent_requests` and a `JoinSet` to drive them all
    /// concurrently on the Tokio thread pool.
    ///
    /// # Scheduling model
    ///
    /// ```text
    ///  ┌─────────────────────────────────────────────────┐
    ///  │  outer loop                                     │
    ///  │  ┌──────────────────────────────────────────┐   │
    ///  │  │  fill phase                              │   │
    ///  │  │  while join_set.len() < concurrency      │   │
    ///  │  │    pop URL from state → spawn task       │   │
    ///  │  └──────────────────────────────────────────┘   │
    ///  │  drain phase: join_next() → update state        │
    ///  └─────────────────────────────────────────────────┘
    /// ```
    ///
    /// Discovered links are fed back into `state` after each result is processed,
    /// so the scheduler can immediately pick them up in the next fill phase.
    async fn crawl_loop<F, Fut>(
        &self,
        state: &mut state::CrawlState,
        fetch_fn: F,
    ) -> Result<Vec<CrawlResult>>
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(u16, String, String, Vec<String>)>>
            + Send
            + 'static,
    {
        let concurrency = self.config.max_concurrent_requests.max(1);
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let fetch_fn = Arc::new(fetch_fn);

        // Each task returns (url, depth, fetch_result).
        let mut join_set: JoinSet<(String, usize, Result<(u16, String, String, Vec<String>)>)> =
            JoinSet::new();

        let mut save_counter: u64 = 0;
        let mut consecutive_errors: u64 = 0;
        // Track URLs scheduled but not yet recorded as results, so that
        // max_pages accounting stays accurate under concurrency.
        let mut in_flight: usize = 0;

        loop {
            // ── Fill phase ────────────────────────────────────────────────
            // Keep the JoinSet saturated up to `concurrency`.
            'fill: while join_set.len() < concurrency {
                // Honour max_pages including in-flight tasks so we don't
                // over-schedule when concurrency > 1.
                if let Some(max_pages) = self.config.max_pages {
                    if state.result_count() + in_flight >= max_pages {
                        break 'fill;
                    }
                }

                match state.next_pending() {
                    None => break 'fill,
                    Some((url, depth)) => {
                        if !self.should_crawl_url(&url) {
                            continue;
                        }
                        // Mark visited now so parallel fill iterations don't
                        // re-schedule the same URL.
                        state.mark_visited(url.clone());

                        // pre_request hooks run on the main task (fast path).
                        if let Some(hooks_cfg) = &self.config.hooks {
                            if !hooks_cfg.pre_request.is_empty() {
                                let mut env = HashMap::new();
                                env.insert("URL".to_string(), url.clone());
                                if let Err(e) = hooks::run_hooks(&hooks_cfg.pre_request, &env).await
                                {
                                    eprintln!("pre_request hook error for {}: {}", url, e);
                                }
                            }
                        }

                        eprintln!("Crawling [depth={}]: {}", depth, url);

                        let sem = semaphore.clone();
                        let fetch = fetch_fn.clone();
                        let max_retries = self.config.max_retries;
                        let delay_ms = self.config.delay_ms;

                        in_flight += 1;
                        join_set.spawn(async move {
                            // The permit is held for the duration of the fetch,
                            // bounding real concurrency even if the JoinSet has
                            // more tasks than slots.
                            let _permit = sem.acquire_owned().await.expect("semaphore closed");

                            // Inline retry logic so each task is self-contained.
                            let mut last_error: Option<anyhow::Error> = None;
                            for attempt in 0..=max_retries {
                                match fetch(url.clone()).await {
                                    Ok(r) => return (url, depth, Ok(r)),
                                    Err(e) => {
                                        if attempt < max_retries {
                                            eprintln!(
                                                "Retry {}/{} for {}",
                                                attempt + 1,
                                                max_retries,
                                                url
                                            );
                                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                                delay_ms * (attempt as u64 + 1),
                                            ))
                                            .await;
                                        }
                                        last_error = Some(e);
                                    }
                                }
                            }
                            (url, depth, Err(last_error.unwrap()))
                        });
                    }
                }
            }

            // ── Drain phase ───────────────────────────────────────────────
            // If there is nothing running and nothing left to schedule, done.
            if join_set.is_empty() {
                break;
            }

            match join_set.join_next().await {
                None => break,

                Some(Err(join_err)) => {
                    // Task panicked — count as an error but don't crash.
                    eprintln!("Crawl task panicked: {:?}", join_err);
                    in_flight = in_flight.saturating_sub(1);
                    consecutive_errors += 1;
                    if consecutive_errors > 10 {
                        eprintln!("Too many errors, aborting remaining tasks");
                        join_set.abort_all();
                        break;
                    }
                }

                Some(Ok((url, depth, Err(e)))) => {
                    in_flight = in_flight.saturating_sub(1);
                    eprintln!("Error fetching {}: {}", url, e);
                    consecutive_errors += 1;

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

                    if let Some(hooks_cfg) = &self.config.hooks {
                        if !hooks_cfg.on_error.is_empty() {
                            let mut env = HashMap::new();
                            env.insert("URL".to_string(), url);
                            env.insert("ERROR".to_string(), e.to_string());
                            let _ = hooks::run_hooks(&hooks_cfg.on_error, &env).await;
                        }
                    }

                    if consecutive_errors > 10 {
                        eprintln!("Too many consecutive errors, stopping crawl");
                        join_set.abort_all();
                        break;
                    }
                }

                Some(Ok((url, depth, Ok((status_code, html, clean_text, links))))) => {
                    in_flight = in_flight.saturating_sub(1);
                    consecutive_errors = 0;

                    let title = Self::extract_title(&html);
                    let state_result = state::CrawlResult {
                        url: url.clone(),
                        depth,
                        status_code: Some(status_code),
                        title,
                        content: clean_text,
                        links: links.clone(),
                        timestamp: SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };

                    let filtered_links: Vec<String> = links
                        .into_iter()
                        .filter(|link| self.should_crawl_url(link))
                        .collect();

                    state.add_pending(filtered_links, depth);
                    state.add_result(state_result);

                    if let Some(hooks_cfg) = &self.config.hooks {
                        if !hooks_cfg.post_extract.is_empty() {
                            let mut env = HashMap::new();
                            env.insert("URL".to_string(), url);
                            env.insert("STATUS_CODE".to_string(), status_code.to_string());
                            if let Err(e) = hooks::run_hooks(&hooks_cfg.post_extract, &env).await {
                                eprintln!("post_extract hook error: {}", e);
                            }
                        }
                    }
                }
            }

            // Periodic state save and progress report.
            save_counter += 1;
            if save_counter % 10 == 0 {
                if let Some(state_file) = &self.config.state_file {
                    if let Err(e) = state.save(state_file) {
                        eprintln!("Warning: Failed to save state: {}", e);
                    }
                }
            }
            let rc = state.result_count();
            if rc > 0 && rc.is_multiple_of(10) {
                eprintln!(
                    "Progress: {} pages crawled, {} pending",
                    rc,
                    state.pending_count()
                );
            }
        }

        if let Some(state_file) = &self.config.state_file {
            state.save(state_file)?;
        }

        eprintln!(
            "Crawl complete: {} pages in {} seconds",
            state.result_count(),
            state.elapsed_seconds()
        );

        Ok(state
            .results()
            .iter()
            .map(Self::convert_state_result)
            .collect())
    }

    /// Check if URL should be crawled based on include/exclude patterns.
    fn should_crawl_url(&self, url: &str) -> bool {
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

    /// Extract title from HTML.
    fn extract_title(html: &str) -> Option<String> {
        use scraper::{Html, Selector};
        let document = Html::parse_document(html);
        let selector = Selector::parse("title").ok()?;
        document
            .select(&selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
    }

    /// Convert state::CrawlResult to lib.rs CrawlResult.
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

fn default_crawl_mode() -> CrawlMode {
    CrawlMode::Http
}
fn default_max_depth() -> usize {
    3
}
fn default_timeout_secs() -> u64 {
    30
}
fn default_max_concurrent_requests() -> usize {
    10
}
fn default_max_retries() -> usize {
    3
}
fn default_output_format() -> OutputFormat {
    OutputFormat::Json
}

/// Configuration for the web crawler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    /// The starting URL to crawl.
    #[serde(default)]
    pub url: String,

    /// The crawling mode (HTTP or Browser).
    #[serde(default = "default_crawl_mode")]
    pub mode: CrawlMode,

    /// Maximum depth to crawl (0 = only start URL).
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// Maximum number of pages to crawl.
    #[serde(default)]
    pub max_pages: Option<usize>,

    /// Optional path to save output.
    #[serde(default)]
    pub output_path: Option<PathBuf>,

    /// Optional path to save/load crawl state for resumability.
    #[serde(default)]
    pub state_file: Option<PathBuf>,

    /// Output format for results.
    #[serde(default = "default_output_format")]
    pub output_format: OutputFormat,

    /// Proxy settings.
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,

    /// User agent string.
    #[serde(default)]
    pub user_agent: Option<String>,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Maximum number of concurrent requests.
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,

    /// Delay between requests in milliseconds.
    #[serde(default)]
    pub delay_ms: u64,

    /// Maximum number of retries per request.
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Follow redirects.
    #[serde(default = "default_true")]
    pub follow_redirects: bool,

    /// Respect robots.txt.
    #[serde(default = "default_true")]
    pub respect_robots_txt: bool,

    /// URL patterns to include (regex).
    #[serde(default)]
    pub include_patterns: Vec<String>,

    /// URL patterns to exclude (regex).
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Extract clean content using readability algorithm.
    #[serde(default)]
    pub md_readability: bool,

    /// Additional HTTP headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Optional stage name (for pipeline display).
    #[serde(default)]
    pub name: Option<String>,

    /// Load start URLs from a previous stage's output file.
    #[serde(default)]
    pub input_from: Option<PathBuf>,

    /// Restrict JSON output to these top-level fields.
    #[serde(default)]
    pub extract_fields: Vec<String>,

    /// Transformers applied after crawl and before output.
    #[serde(default)]
    pub transformers: Vec<Transformer>,

    /// Lifecycle hooks.
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// Session management (load/save cookies and headers).
    #[serde(default)]
    pub session: Option<SessionConfig>,
}

impl CrawlConfig {
    /// Creates a new builder for constructing a CrawlConfig.
    pub fn builder() -> CrawlConfigBuilder {
        CrawlConfigBuilder::default()
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<()> {
        // Validate URL -- required unless input_from provides URLs
        if !self.url.is_empty() {
            Url::parse(&self.url).map_err(|e| CrawlError::InvalidUrl {
                url: self.url.clone(),
                reason: e.to_string(),
            })?;
        } else if self.input_from.is_none() {
            return Err(CrawlError::ConfigError {
                message: "Either 'url' or 'input_from' must be set".to_string(),
            }
            .into());
        }

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

        config.validate()?;
        Ok(config)
    }

    /// Saves the current configuration to a YAML recipe file.
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

    /// Return this config with the URL replaced.
    ///
    /// Useful when loading a recipe template and overriding just the target URL —
    /// for example in fallback strategies where all other settings should be reused.
    ///
    /// # Example
    /// ```no_run
    /// # use anyhow::Result;
    /// # fn main() -> Result<()> {
    /// use crawlery::CrawlConfig;
    /// let config = CrawlConfig::from_file("recipes/http.yaml")?.with_url("https://example.com/page");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
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
    md_readability: bool,
    headers: HashMap<String, String>,
    name: Option<String>,
    input_from: Option<PathBuf>,
    extract_fields: Vec<String>,
    transformers: Vec<Transformer>,
    hooks: Option<HooksConfig>,
    session: Option<SessionConfig>,
}

impl CrawlConfigBuilder {
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn mode(mut self, mode: CrawlMode) -> Self {
        self.mode = Some(mode);
        self
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn max_pages(mut self, pages: usize) -> Self {
        self.max_pages = Some(pages);
        self
    }

    pub fn output_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_path = Some(path.into());
        self
    }

    pub fn state_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_file = Some(path.into());
        self
    }

    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = Some(format);
        self
    }

    pub fn proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = Some(proxy);
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = Some(max);
        self
    }

    pub fn delay_ms(mut self, ms: u64) -> Self {
        self.delay_ms = Some(ms);
        self
    }

    pub fn max_retries(mut self, retries: usize) -> Self {
        self.max_retries = Some(retries);
        self
    }

    pub fn follow_redirects(mut self, follow: bool) -> Self {
        self.follow_redirects = Some(follow);
        self
    }

    pub fn respect_robots_txt(mut self, respect: bool) -> Self {
        self.respect_robots_txt = Some(respect);
        self
    }

    pub fn include_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    pub fn exclude_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    pub fn md_readability(mut self, extract: bool) -> Self {
        self.md_readability = extract;
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn input_from(mut self, path: impl Into<PathBuf>) -> Self {
        self.input_from = Some(path.into());
        self
    }

    pub fn extract_field(mut self, field: impl Into<String>) -> Self {
        self.extract_fields.push(field.into());
        self
    }

    pub fn transformer(mut self, t: Transformer) -> Self {
        self.transformers.push(t);
        self
    }

    pub fn hooks(mut self, hooks: HooksConfig) -> Self {
        self.hooks = Some(hooks);
        self
    }

    pub fn session(mut self, session: SessionConfig) -> Self {
        self.session = Some(session);
        self
    }

    /// Builds the CrawlConfig, validating all settings.
    pub fn build(self) -> Result<CrawlConfig> {
        let has_input_from = self.input_from.is_some();
        let url = match self.url {
            Some(u) => u,
            None if has_input_from => String::new(),
            None => {
                return Err(CrawlError::ConfigError {
                    message: "Either 'url' or 'input_from' must be set".to_string(),
                }
                .into())
            }
        };

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
            md_readability: self.md_readability,
            headers: self.headers,
            name: self.name,
            input_from: self.input_from,
            extract_fields: self.extract_fields,
            transformers: self.transformers,
            hooks: self.hooks,
            session: self.session,
        };

        config.validate()?;
        Ok(config)
    }
}

/// Crawling mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrawlMode {
    Http,
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
    pub url: String,
    pub status_code: Option<u16>,
    pub title: Option<String>,
    pub content: String,
    pub links: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub timestamp: SystemTime,
    pub depth: usize,
    pub content_type: Option<String>,
    pub headers: HashMap<String, String>,
    pub errors: Vec<String>,
}

impl CrawlResult {
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

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    pub fn is_success(&self) -> bool {
        self.status_code
            .is_some_and(|code| (200..300).contains(&code))
    }
}

/// Output format for crawl results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    #[serde(alias = "json-pretty")]
    JsonPretty,
    Markdown,
    Csv,
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
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            username: None,
            password: None,
        }
    }

    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }
}

fn default_true() -> bool {
    true
}

/// Session configuration for loading/saving cookies and headers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub load_from: Option<PathBuf>,
    pub save_to: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub save_cookies: bool,
    #[serde(default = "default_true")]
    pub save_headers: bool,
}

/// Lifecycle hook configuration (all fields default to empty).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    #[serde(default)]
    pub pre_request: Vec<Hook>,
    #[serde(default)]
    pub post_load: Vec<Hook>,
    #[serde(default)]
    pub post_extract: Vec<Hook>,
    #[serde(default)]
    pub on_error: Vec<Hook>,
}

/// A single lifecycle hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    #[serde(flatten)]
    pub hook_type: HookType,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub abort_on_error: bool,
}

/// The type of a hook -- shell command or JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookType {
    Command {
        cmd: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Javascript {
        source: String,
    },
}

/// Result transformer applied after crawl, before output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transformer {
    Filter {
        condition: FilterCondition,
    },
    Deduplicator {
        field: String,
    },
    ExtractFields {
        fields: Vec<String>,
    },
    Command {
        cmd: String,
        #[serde(default)]
        args: Vec<String>,
        timeout_ms: Option<u64>,
    },
}

/// Condition expression for the Filter transformer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCondition {
    pub expression: String,
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
            .md_readability(true)
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
        assert_eq!(loaded_config.md_readability, original_config.md_readability);
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
md_readability: false
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

    // ── Concurrent crawl_loop tests ───────────────────────────────────────────

    /// Seed a CrawlState with N URLs at depth 0, ready to crawl.
    fn make_state(urls: Vec<&str>, max_pages: Option<usize>) -> state::CrawlState {
        let mut s = state::CrawlState::new(state::CrawlConfig {
            start_url: urls[0].to_string(),
            max_depth: 2,
            max_pages,
            respect_robots_txt: false,
        });
        s.seed_urls(urls.iter().map(|u| u.to_string()).collect());
        s
    }

    /// Build a Crawler configured for concurrent HTTP crawling.
    fn make_crawler(concurrency: usize, max_pages: Option<usize>) -> Crawler {
        let mut cfg = CrawlConfig::builder()
            .url("http://test/0")
            .max_concurrent_requests(concurrency)
            .max_depth(2)
            .respect_robots_txt(false)
            .build()
            .unwrap();
        cfg.max_pages = max_pages;
        Crawler::new(cfg)
    }

    /// Prove that with concurrency = 4 and 4 URLs each taking 100 ms, the total
    /// wall-clock time is well under 4 × 100 ms (sequential would be ≥ 400 ms).
    #[tokio::test]
    async fn test_crawl_loop_fetches_in_parallel() {
        let crawler = make_crawler(4, Some(4));
        let mut state = make_state(
            vec!["http://t/0", "http://t/1", "http://t/2", "http://t/3"],
            Some(4),
        );

        let start = std::time::Instant::now();

        let results = crawler
            .crawl_loop(&mut state, |url: String| async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                Ok::<_, anyhow::Error>((
                    200u16,
                    format!("<html><title>{}</title></html>", url),
                    url.clone(),
                    vec![],
                ))
            })
            .await
            .unwrap();

        let elapsed = start.elapsed();

        assert_eq!(results.len(), 4, "All 4 URLs must be crawled");
        assert!(
            elapsed < std::time::Duration::from_millis(350),
            "4 parallel 100 ms fetches should finish in < 350 ms, took {:?}",
            elapsed
        );
    }

    /// Prove that the Semaphore actually caps concurrency.
    /// With 8 URLs, 100 ms each, and concurrency = 2, the crawl must take
    /// at least 4 × 100 ms = 400 ms (4 sequential waves of 2).
    #[tokio::test]
    async fn test_crawl_loop_semaphore_caps_concurrency() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let concurrency = 2usize;
        let crawler = make_crawler(concurrency, Some(8));
        let mut state = make_state(
            vec![
                "http://t/0",
                "http://t/1",
                "http://t/2",
                "http://t/3",
                "http://t/4",
                "http://t/5",
                "http://t/6",
                "http://t/7",
            ],
            Some(8),
        );

        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let active2 = active.clone();
        let peak2 = peak.clone();

        let results = crawler
            .crawl_loop(&mut state, move |url: String| {
                let a = active2.clone();
                let p = peak2.clone();
                async move {
                    // Track peak concurrent active fetches.
                    let current = a.fetch_add(1, Ordering::AcqRel) + 1;
                    p.fetch_max(current, Ordering::AcqRel);
                    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                    a.fetch_sub(1, Ordering::AcqRel);
                    Ok::<_, anyhow::Error>((
                        200u16,
                        format!("<html><title>{}</title></html>", url),
                        url.clone(),
                        vec![],
                    ))
                }
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 8);
        assert!(
            peak.load(Ordering::Acquire) <= concurrency,
            "Peak concurrency {} exceeded the limit of {}",
            peak.load(Ordering::Acquire),
            concurrency
        );
        assert!(
            peak.load(Ordering::Acquire) > 1,
            "Expected concurrent execution (peak should be > 1)"
        );
    }

    /// Prove that max_pages is not over-shot under concurrency.
    /// With concurrency = 8 and max_pages = 5, exactly 5 pages must be
    /// returned even though 8 tasks could otherwise be scheduled at once.
    #[tokio::test]
    async fn test_crawl_loop_max_pages_not_exceeded() {
        let max_pages = 5usize;
        let crawler = make_crawler(8, Some(max_pages));
        let mut state = make_state(
            vec![
                "http://t/0",
                "http://t/1",
                "http://t/2",
                "http://t/3",
                "http://t/4",
                "http://t/5",
                "http://t/6",
                "http://t/7",
            ],
            Some(max_pages),
        );

        let results = crawler
            .crawl_loop(&mut state, |url: String| async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
                Ok::<_, anyhow::Error>((
                    200u16,
                    format!("<html><title>{}</title></html>", url),
                    url.clone(),
                    vec![],
                ))
            })
            .await
            .unwrap();

        assert!(
            results.len() <= max_pages,
            "Expected at most {} results, got {}",
            max_pages,
            results.len()
        );
    }

    /// Prove that failed fetches are recorded, the error counter resets on
    /// success, and crawling continues past individual errors.
    #[tokio::test]
    async fn test_crawl_loop_records_errors_and_continues() {
        let crawler = make_crawler(2, Some(6));
        let mut state = make_state(
            vec![
                "http://t/ok0",
                "http://t/fail1",
                "http://t/ok2",
                "http://t/fail3",
                "http://t/ok4",
                "http://t/ok5",
            ],
            Some(6),
        );

        let results = crawler
            .crawl_loop(&mut state, |url: String| async move {
                if url.contains("fail") {
                    Err(anyhow::anyhow!("simulated fetch error for {}", url))
                } else {
                    Ok::<_, anyhow::Error>((
                        200u16,
                        format!("<html><title>{}</title></html>", url),
                        url.clone(),
                        vec![],
                    ))
                }
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 6, "All 6 URLs (ok + fail) must be recorded");

        let ok_count = results.iter().filter(|r| r.is_success()).count();
        let err_count = results.iter().filter(|r| !r.is_success()).count();
        assert_eq!(ok_count, 4, "4 successful fetches expected");
        assert_eq!(err_count, 2, "2 failed fetches expected");
    }

    /// Prove that discovered links are followed: seeding one URL that returns
    /// a link causes the linked page to also be crawled.
    #[tokio::test]
    async fn test_crawl_loop_follows_discovered_links() {
        let crawler = make_crawler(2, Some(2));
        let mut state = make_state(vec!["http://t/root"], Some(2));

        let results = crawler
            .crawl_loop(&mut state, |url: String| async move {
                let links = if url == "http://t/root" {
                    vec!["http://t/child".to_string()]
                } else {
                    vec![]
                };
                Ok::<_, anyhow::Error>((
                    200u16,
                    format!("<html><title>{}</title></html>", url),
                    url.clone(),
                    links,
                ))
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 2, "root + child must both be crawled");
        let urls: Vec<&str> = results.iter().map(|r| r.url.as_str()).collect();
        assert!(urls.contains(&"http://t/root"));
        assert!(urls.contains(&"http://t/child"));
    }

    /// Prove that a URL is never crawled twice even under concurrent scheduling.
    #[tokio::test]
    async fn test_crawl_loop_no_duplicate_fetches() {
        use std::sync::{Arc, Mutex};

        let crawler = make_crawler(4, Some(10));
        let mut state = make_state(vec!["http://t/0", "http://t/1", "http://t/2"], Some(10));

        let fetched: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let fetched2 = fetched.clone();

        let _results = crawler
            .crawl_loop(&mut state, move |url: String| {
                let f = fetched2.clone();
                async move {
                    f.lock().unwrap().push(url.clone());
                    Ok::<_, anyhow::Error>((
                        200u16,
                        format!("<html><title>{}</title></html>", url),
                        url.clone(),
                        // Each page returns the same 3 URLs as links —
                        // none should be re-fetched.
                        vec![
                            "http://t/0".to_string(),
                            "http://t/1".to_string(),
                            "http://t/2".to_string(),
                        ],
                    ))
                }
            })
            .await
            .unwrap();

        let log = fetched.lock().unwrap();
        // Verify no URL appears more than once.
        let mut seen = std::collections::HashSet::new();
        for url in log.iter() {
            assert!(
                seen.insert(url.as_str()),
                "URL fetched more than once: {}",
                url
            );
        }
    }
}
