//! Crawl state management module for resumable web crawling.
//!
//! This module provides state management functionality that enables web crawling sessions
//! to be paused, saved, and resumed later. It tracks visited URLs, pending URLs to crawl,
//! and collected results, all of which can be serialized to JSON for persistence.
//!
//! # Features
//!
//! - **Resumable crawling** - Save and restore crawl progress at any time
//! - **Duplicate detection** - Automatically prevents re-crawling visited URLs
//! - **Depth tracking** - Maintains crawl depth for each URL
//! - **Result collection** - Stores all crawl results with metadata
//! - **Progress monitoring** - Track visited, pending, and result counts
//!
//! # Example
//!
//! ```no_run
//! use crawlery::state::{CrawlState, CrawlConfig, CrawlResult};
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Create a new crawl state
//! let config = CrawlConfig {
//!     start_url: "https://example.com".to_string(),
//!     max_depth: 2,
//!     max_pages: Some(100),
//!     respect_robots_txt: true,
//! };
//! let mut state = CrawlState::new(config);
//!
//! // Process URLs
//! while let Some((url, depth)) = state.next_pending() {
//!     // Crawl the URL (not shown)
//!     state.mark_visited(url.clone());
//!
//!     // Add discovered links
//!     let links = vec!["https://example.com/page1".to_string()];
//!     state.add_pending(links, depth);
//! }
//!
//! // Save state for later resumption
//! state.save("crawl_state.json")?;
//!
//! // Later, resume from saved state
//! let mut resumed_state = CrawlState::load("crawl_state.json")?;
//! # Ok(())
//! # }
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use url::Url;

/// Configuration for the crawl operation.
///
/// Contains the core settings that define the scope and behavior of a crawl session.
/// This configuration is stored with the crawl state to maintain consistency when
/// resuming a crawl.
///
/// # Examples
///
/// ```
/// use crawlery::state::CrawlConfig;
///
/// let config = CrawlConfig {
///     start_url: "https://example.com".to_string(),
///     max_depth: 3,
///     max_pages: Some(1000),
///     respect_robots_txt: true,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    /// The starting URL for the crawl.
    ///
    /// This is the initial URL from which the crawl begins. It's added to the
    /// pending queue when the state is first created.
    pub start_url: String,

    /// Maximum depth to crawl from the start URL.
    ///
    /// Depth 0 means only the start URL itself. Depth 1 includes links from
    /// the start URL, depth 2 includes links from those pages, and so on.
    pub max_depth: usize,

    /// Optional maximum number of pages to crawl.
    ///
    /// When set, the crawl will stop after this many pages have been successfully
    /// crawled. Use `None` for unlimited crawling (bounded only by max_depth).
    pub max_pages: Option<usize>,

    /// Whether to respect robots.txt rules.
    ///
    /// When `true`, the crawler should check and honor robots.txt directives.
    /// This is stored in the configuration but enforced by the crawler implementation.
    pub respect_robots_txt: bool,
}

/// Result of crawling a single URL.
///
/// Contains all the information extracted from a crawled page, including content,
/// metadata, discovered links, and status information. Results are stored in the
/// crawl state and can be serialized for later analysis.
///
/// # Examples
///
/// ```
/// use crawlery::state::CrawlResult;
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// let result = CrawlResult {
///     url: "https://example.com".to_string(),
///     depth: 0,
///     status_code: Some(200),
///     title: Some("Example Domain".to_string()),
///     content: "This is example content".to_string(),
///     links: vec!["https://example.com/page1".to_string()],
///     timestamp: SystemTime::now()
///         .duration_since(UNIX_EPOCH)
///         .unwrap()
///         .as_secs(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    /// The URL that was crawled.
    pub url: String,

    /// The depth level at which this URL was discovered.
    ///
    /// The start URL has depth 0, links from it have depth 1, and so on.
    pub depth: usize,

    /// HTTP status code returned by the server.
    ///
    /// May be `None` if the request failed before receiving a response
    /// or if using browser-based crawling where status is unavailable.
    pub status_code: Option<u16>,

    /// The page title extracted from the HTML.
    ///
    /// Typically extracted from the `<title>` tag. May be `None` if
    /// the page has no title or if extraction failed.
    pub title: Option<String>,

    /// The page content (cleaned text or HTML).
    ///
    /// This contains the main content of the page, typically with scripts,
    /// styles, and navigation elements removed for RAG applications.
    pub content: String,

    /// List of links discovered on this page.
    ///
    /// These are the absolute URLs found in anchor tags on the page.
    pub links: Vec<String>,

    /// Unix timestamp (seconds since epoch) when the page was crawled.
    ///
    /// This can be used to track when data was collected and to identify
    /// stale content that may need re-crawling.
    pub timestamp: u64,
}

/// Pending URL with its depth
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingUrl {
    url: String,
    depth: usize,
}

/// State management for web crawling with resumption support.
///
/// `CrawlState` maintains all the information needed to track and resume a web crawl,
/// including which URLs have been visited, which are pending, and what results have
/// been collected. The state can be serialized to JSON and saved to disk, enabling
/// long-running crawls to be paused and resumed across process restarts.
///
/// # Thread Safety
///
/// `CrawlState` is not thread-safe by default. If you need concurrent access,
/// wrap it in `Arc<Mutex<CrawlState>>` or `Arc<RwLock<CrawlState>>`.
///
/// # Examples
///
/// ```no_run
/// use crawlery::state::{CrawlState, CrawlConfig, CrawlResult};
///
/// # fn main() -> anyhow::Result<()> {
/// let config = CrawlConfig {
///     start_url: "https://example.com".to_string(),
///     max_depth: 2,
///     max_pages: Some(50),
///     respect_robots_txt: true,
/// };
///
/// let mut state = CrawlState::new(config);
///
/// // Crawl URLs
/// while let Some((url, depth)) = state.next_pending() {
///     println!("Crawling: {} at depth {}", url, depth);
///     state.mark_visited(url);
///     // ... fetch and process the URL ...
/// }
///
/// println!("Crawled {} pages", state.result_count());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct CrawlState {
    config: CrawlConfig,
    visited: HashSet<String>,
    #[serde(with = "vec_deque_serde")]
    pending: VecDeque<PendingUrl>,
    results: Vec<CrawlResult>,
    start_time: u64,
}

impl CrawlState {
    /// Normalizes a URL by removing the fragment identifier.
    ///
    /// This ensures that URLs like `https://example.com/page#section1` and
    /// `https://example.com/page#section2` are treated as the same URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL string to normalize
    ///
    /// # Returns
    ///
    /// Returns the normalized URL string with the fragment removed.
    /// If parsing fails, returns the original URL.
    fn normalize_url(url: &str) -> String {
        match Url::parse(url) {
            Ok(mut parsed_url) => {
                parsed_url.set_fragment(None);
                parsed_url.to_string()
            }
            Err(_) => url.to_string(),
        }
    }

    /// Creates a new crawl state with the given configuration.
    ///
    /// Initializes a new crawl session by setting up the pending queue with the start URL,
    /// initializing empty collections for visited URLs and results, and recording the
    /// start time.
    ///
    /// # Arguments
    ///
    /// * `config` - The crawl configuration specifying start URL, depth limits, etc.
    ///
    /// # Returns
    ///
    /// Returns a new `CrawlState` ready to begin crawling from the configured start URL.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig};
    ///
    /// let config = CrawlConfig {
    ///     start_url: "https://example.com".to_string(),
    ///     max_depth: 2,
    ///     max_pages: None,
    ///     respect_robots_txt: true,
    /// };
    ///
    /// let state = CrawlState::new(config);
    /// assert_eq!(state.visited_count(), 0);
    /// assert_eq!(state.pending_count(), 1); // Start URL is pending
    /// ```
    pub fn new(config: CrawlConfig) -> Self {
        let mut pending = VecDeque::new();
        pending.push_back(PendingUrl {
            url: Self::normalize_url(&config.start_url),
            depth: 0,
        });

        let start_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            config,
            visited: HashSet::new(),
            pending,
            results: Vec::new(),
            start_time,
        }
    }


    /// Replace the pending queue with a set of seed URLs at depth 0.
    /// Used when `input_from` provides start URLs for a pipeline stage.
    pub fn seed_urls(&mut self, urls: Vec<String>) {
        self.pending.clear();
        for url in urls {
            let normalized = Self::normalize_url(&url);
            if !self.is_visited(&normalized) {
                self.pending.push_back(PendingUrl { url: normalized, depth: 0 });
            }
        }
    }

    /// Loads crawl state from a JSON file for resumption.
    ///
    /// Deserializes a previously saved crawl state from a JSON file, allowing
    /// the crawl to continue from where it left off.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file containing the saved state
    ///
    /// # Returns
    ///
    /// Returns the deserialized `CrawlState` ready to resume crawling.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read
    /// - The JSON is invalid or doesn't match the expected format
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawlery::state::CrawlState;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let state = CrawlState::load("crawl_state.json")?;
    /// println!("Resumed crawl with {} results", state.result_count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let state: CrawlState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Saves crawl state to a JSON file.
    ///
    /// Serializes the entire crawl state to pretty-printed JSON and writes it to a file.
    /// This enables resuming the crawl later or analyzing the state externally.
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the state file should be written
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The state cannot be serialized to JSON
    /// - The file cannot be created or written
    /// - There's insufficient disk space
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use crawlery::state::{CrawlState, CrawlConfig};
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let config = CrawlConfig {
    ///     start_url: "https://example.com".to_string(),
    ///     max_depth: 2,
    ///     max_pages: Some(100),
    ///     respect_robots_txt: true,
    /// };
    /// let state = CrawlState::new(config);
    ///
    /// // Save the state
    /// state.save("crawl_state.json")?;
    /// println!("State saved successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Marks a URL as visited.
    ///
    /// Adds the URL to the visited set to prevent re-crawling it in the future.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to mark as visited
    pub fn mark_visited(&mut self, url: String) {
        let normalized = Self::normalize_url(&url);
        self.visited.insert(normalized);
    }

    /// Checks if a URL has been visited.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the URL has been marked as visited, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig};
    ///
    /// # let config = CrawlConfig {
    /// #     start_url: "https://example.com".to_string(),
    /// #     max_depth: 2,
    /// #     max_pages: None,
    /// #     respect_robots_txt: true,
    /// # };
    /// let mut state = CrawlState::new(config);
    ///
    /// assert!(!state.is_visited("https://example.com"));
    /// state.mark_visited("https://example.com".to_string());
    /// assert!(state.is_visited("https://example.com"));
    /// ```
    pub fn is_visited(&self, url: &str) -> bool {
        let normalized = Self::normalize_url(url);
        self.visited.contains(&normalized)
    }

    /// Adds URLs to the pending queue with depth tracking.
    ///
    /// This method adds newly discovered URLs to the pending queue, respecting max_depth
    /// constraints and automatically deduplicating against visited and already-pending URLs.
    ///
    /// # Arguments
    ///
    /// * `urls` - Vector of URLs to add to the pending queue
    /// * `depth` - The current depth (URLs will be added at depth + 1)
    ///
    /// # Behavior
    ///
    /// - URLs are added at `depth + 1`
    /// - URLs beyond `max_depth` are skipped
    /// - Already-visited URLs are skipped
    /// - Already-pending URLs are skipped
    pub fn add_pending(&mut self, urls: Vec<String>, depth: usize) {
        // Check if we've reached max depth
        if depth >= self.config.max_depth {
            return;
        }

        for url in urls {
            let normalized = Self::normalize_url(&url);

            // Skip if already visited or already in pending queue
            if self.is_visited(&normalized) || self.is_pending(&normalized) {
                continue;
            }

            self.pending.push_back(PendingUrl {
                url: normalized,
                depth: depth + 1,
            });
        }
    }

    /// Gets the next pending URL to crawl.
    ///
    /// Pops the next URL from the pending queue and returns it with its depth.
    /// This method automatically:
    /// - Enforces the max_pages limit (returns None if limit reached)
    /// - Skips URLs that have been visited since they were added
    /// - Returns None when no more URLs are available
    ///
    /// # Returns
    ///
    /// Returns `Some((url, depth))` if there's a URL to crawl, or `None` if:
    /// - The max_pages limit has been reached
    /// - There are no more pending URLs
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig};
    ///
    /// # let config = CrawlConfig {
    /// #     start_url: "https://example.com".to_string(),
    /// #     max_depth: 2,
    /// #     max_pages: None,
    /// #     respect_robots_txt: true,
    /// # };
    /// let mut state = CrawlState::new(config);
    ///
    /// if let Some((url, depth)) = state.next_pending() {
    ///     println!("Next URL to crawl: {} at depth {}", url, depth);
    ///     state.mark_visited(url);
    /// }
    /// ```
    pub fn next_pending(&mut self) -> Option<(String, usize)> {
        // Check max_pages limit
        if let Some(max_pages) = self.config.max_pages {
            if self.results.len() >= max_pages {
                return None;
            }
        }

        // Skip already-visited URLs
        while let Some(p) = self.pending.pop_front() {
            if !self.is_visited(&p.url) {
                return Some((p.url, p.depth));
            }
        }

        None
    }

    /// Checks if a URL is in the pending queue.
    fn is_pending(&self, url: &str) -> bool {
        let normalized = Self::normalize_url(url);
        self.pending.iter().any(|p| p.url == normalized)
    }

    /// Adds a crawl result to the state.
    ///
    /// Stores the result of crawling a URL, including its content, links, and metadata.
    /// Results are kept in memory and can be saved to disk along with the rest of the state.
    ///
    /// # Arguments
    ///
    /// * `result` - The crawl result to add
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig, CrawlResult};
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// # let config = CrawlConfig {
    /// #     start_url: "https://example.com".to_string(),
    /// #     max_depth: 2,
    /// #     max_pages: None,
    /// #     respect_robots_txt: true,
    /// # };
    /// let mut state = CrawlState::new(config);
    ///
    /// let result = CrawlResult {
    ///     url: "https://example.com".to_string(),
    ///     depth: 0,
    ///     status_code: Some(200),
    ///     title: Some("Example".to_string()),
    ///     content: "Content here".to_string(),
    ///     links: vec![],
    ///     timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    /// };
    ///
    /// state.add_result(result);
    /// assert_eq!(state.result_count(), 1);
    /// ```
    pub fn add_result(&mut self, result: CrawlResult) {
        self.results.push(result);
    }

    /// Returns all crawl results collected so far.
    ///
    /// # Returns
    ///
    /// Returns a slice containing all stored `CrawlResult` entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig};
    ///
    /// # let config = CrawlConfig {
    /// #     start_url: "https://example.com".to_string(),
    /// #     max_depth: 2,
    /// #     max_pages: None,
    /// #     respect_robots_txt: true,
    /// # };
    /// let state = CrawlState::new(config);
    ///
    /// for result in state.results() {
    ///     println!("Crawled: {}", result.url);
    /// }
    /// ```
    pub fn results(&self) -> &[CrawlResult] {
        &self.results
    }

    /// Returns a reference to the crawl configuration.
    ///
    /// # Returns
    ///
    /// Returns a reference to the `CrawlConfig` used by this state.
    pub fn config(&self) -> &CrawlConfig {
        &self.config
    }

    /// Returns the number of pending URLs (excluding already-visited URLs).
    ///
    /// This counts only URLs that haven't been visited yet. URLs in the pending
    /// queue that have already been marked as visited are not counted.
    ///
    /// # Returns
    ///
    /// Returns the count of unvisited pending URLs.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig};
    ///
    /// # let config = CrawlConfig {
    /// #     start_url: "https://example.com".to_string(),
    /// #     max_depth: 2,
    /// #     max_pages: None,
    /// #     respect_robots_txt: true,
    /// # };
    /// let mut state = CrawlState::new(config);
    ///
    /// println!("{} URLs pending", state.pending_count());
    /// ```
    pub fn pending_count(&self) -> usize {
        self.pending
            .iter()
            .filter(|p| !self.is_visited(&p.url))
            .count()
    }

    /// Returns the number of visited URLs.
    ///
    /// # Returns
    ///
    /// Returns the total count of URLs marked as visited.
    pub fn visited_count(&self) -> usize {
        self.visited.len()
    }

    /// Returns the number of crawl results collected.
    ///
    /// # Returns
    ///
    /// Returns the count of stored `CrawlResult` entries.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Checks if there are more URLs to crawl.
    ///
    /// # Returns
    ///
    /// Returns `true` if the pending queue is not empty, `false` otherwise.
    ///
    /// Note: This returns `true` even if all pending URLs have been visited.
    /// Use `next_pending()` to get the actual next URL to crawl.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Returns the elapsed time since the crawl started (in seconds).
    ///
    /// # Returns
    ///
    /// Returns the number of seconds elapsed since the state was created.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawlery::state::{CrawlState, CrawlConfig};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// # let config = CrawlConfig {
    /// #     start_url: "https://example.com".to_string(),
    /// #     max_depth: 2,
    /// #     max_pages: None,
    /// #     respect_robots_txt: true,
    /// # };
    /// let state = CrawlState::new(config);
    ///
    /// thread::sleep(Duration::from_secs(1));
    /// assert!(state.elapsed_seconds() >= 1);
    /// ```
    pub fn elapsed_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - self.start_time
    }
}

// Custom serialization for VecDeque (serialize as Vec)
mod vec_deque_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::VecDeque;

    pub fn serialize<S, T>(deque: &VecDeque<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        let vec: Vec<&T> = deque.iter().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<VecDeque<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let vec: Vec<T> = Vec::deserialize(deserializer)?;
        Ok(VecDeque::from(vec))
    }
}
