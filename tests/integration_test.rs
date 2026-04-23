//! Integration tests for Crawlery crawler components

use crawlery::{CrawlConfig, CrawlMode, Crawler, OutputFormat};

#[tokio::test]
async fn test_crawler_config_builder() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(2)
        .max_pages(10)
        .delay_ms(100)
        .timeout_secs(30)
        .respect_robots_txt(false)
        .output_format(OutputFormat::Json)
        .build();

    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.url, "https://example.com");
    assert_eq!(config.max_depth, 2);
    assert_eq!(config.max_pages, Some(10));
    assert_eq!(config.delay_ms, 100);
}

#[tokio::test]
async fn test_crawler_config_builder_missing_url() {
    let config = CrawlConfig::builder()
        .mode(CrawlMode::Http)
        .max_depth(2)
        .build();

    assert!(config.is_err());
}

#[tokio::test]
async fn test_crawler_creation() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .build()
        .unwrap();

    let crawler = Crawler::new(config);
    assert_eq!(crawler.config().url, "https://example.com");
    assert_eq!(crawler.config().max_depth, 1);
}

#[tokio::test]
async fn test_state_management() {
    use crawlery::state::{CrawlConfig, CrawlState};

    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().unwrap();
    let state_path = temp_file.path();

    // Create and save state
    let config = CrawlConfig {
        start_url: "https://example.com".to_string(),
        max_depth: 2,
        max_pages: Some(50),
        respect_robots_txt: true,
    };

    let mut state = CrawlState::new(config);
    state.mark_visited("https://example.com".to_string());
    state.add_pending(
        vec![
            "https://example.com/page1".to_string(),
            "https://example.com/page2".to_string(),
        ],
        0,
    );

    assert!(state.save(state_path).is_ok());

    // Load state
    let loaded_state = CrawlState::load(state_path);
    assert!(loaded_state.is_ok());

    let loaded_state = loaded_state.unwrap();
    assert_eq!(loaded_state.visited_count(), 1);
    assert_eq!(loaded_state.pending_count(), 2);
    assert!(loaded_state.is_visited("https://example.com"));
}

#[tokio::test]
async fn test_http_crawler_basic() {
    use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};

    let config = HttpCrawlerConfig {
        user_agent: "Test/1.0".to_string(),
        delay_ms: 0,
        timeout_secs: 10,
        proxies: vec![],
        respect_robots_txt: false,
        extra_headers: std::collections::HashMap::new(),
        initial_cookies: vec![],
    };

    let crawler = HttpCrawler::new(config);
    assert!(crawler.is_ok());
}

#[tokio::test]
async fn test_crawler_with_state_file() {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new().unwrap();
    let state_path = temp_file.path().to_path_buf();

    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .max_pages(5)
        .state_file(state_path.clone())
        .respect_robots_txt(false)
        .timeout_secs(5)
        .build()
        .unwrap();

    let crawler = Crawler::new(config);

    // Verify crawler was created successfully
    assert_eq!(crawler.config().state_file, Some(state_path));
}

#[test]
fn test_crawl_mode_parsing() {
    use std::str::FromStr;

    assert!(matches!(
        CrawlMode::from_str("http").unwrap(),
        CrawlMode::Http
    ));
    assert!(matches!(
        CrawlMode::from_str("browser").unwrap(),
        CrawlMode::Browser
    ));
    assert!(CrawlMode::from_str("invalid").is_err());
}

#[test]
fn test_output_format_parsing() {
    use std::str::FromStr;

    assert!(matches!(
        OutputFormat::from_str("json").unwrap(),
        OutputFormat::Json
    ));
    assert!(matches!(
        OutputFormat::from_str("json-pretty").unwrap(),
        OutputFormat::JsonPretty
    ));
    assert!(matches!(
        OutputFormat::from_str("markdown").unwrap(),
        OutputFormat::Markdown
    ));
    assert!(matches!(
        OutputFormat::from_str("csv").unwrap(),
        OutputFormat::Csv
    ));
    assert!(matches!(
        OutputFormat::from_str("text").unwrap(),
        OutputFormat::Text
    ));
}

#[test]
fn test_url_filtering() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .include_pattern(r"^https://example\.com/blog/.*")
        .exclude_pattern(r".*\.pdf$")
        .build()
        .unwrap();

    assert_eq!(config.include_patterns.len(), 1);
    assert_eq!(config.exclude_patterns.len(), 1);
}

#[test]
fn test_config_validation() {
    // Valid config
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(3)
        .build();
    assert!(config.is_ok());

    // Invalid regex pattern
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .include_pattern("[invalid regex")
        .build();
    assert!(config.is_err());
}

#[tokio::test]
async fn test_multiple_crawl_modes() {
    // HTTP mode
    let http_config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .build()
        .unwrap();

    let http_crawler = Crawler::new(http_config);
    assert!(matches!(http_crawler.config().mode, CrawlMode::Http));

    // Browser mode
    let browser_config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Browser)
        .max_depth(1)
        .build()
        .unwrap();

    let browser_crawler = Crawler::new(browser_config);
    assert!(matches!(browser_crawler.config().mode, CrawlMode::Browser));
}

#[test]
fn test_proxy_config() {
    use crawlery::ProxyConfig;

    let proxy = ProxyConfig::new("http://proxy.example.com:8080");
    assert_eq!(proxy.url, "http://proxy.example.com:8080");
    assert!(proxy.username.is_none());
    assert!(proxy.password.is_none());

    let proxy_with_auth =
        ProxyConfig::new("http://proxy.example.com:8080").with_auth("user", "pass");
    assert_eq!(proxy_with_auth.username, Some("user".to_string()));
    assert_eq!(proxy_with_auth.password, Some("pass".to_string()));
}

#[test]
fn test_crawl_result_creation() {
    use crawlery::CrawlResult;

    let result = CrawlResult::new(
        "https://example.com".to_string(),
        "Test content".to_string(),
        0,
    );

    assert_eq!(result.url, "https://example.com");
    assert_eq!(result.content, "Test content");
    assert_eq!(result.depth, 0);
    assert_eq!(result.link_count(), 0);
}

#[tokio::test]
async fn test_concurrent_request_config() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_concurrent_requests(5)
        .delay_ms(1000)
        .build()
        .unwrap();

    assert_eq!(config.max_concurrent_requests, 5);
    assert_eq!(config.delay_ms, 1000);
}

#[tokio::test]
async fn test_header_configuration() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .header("X-Custom-Header", "custom-value")
        .header("Authorization", "Bearer token")
        .build()
        .unwrap();

    assert_eq!(config.headers.len(), 2);
    assert_eq!(
        config.headers.get("X-Custom-Header"),
        Some(&"custom-value".to_string())
    );
}

#[test]
fn test_md_readability_configuration() {
    // Default should be false (raw HTML)
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .build()
        .unwrap();

    assert_eq!(config.md_readability, false);

    // When enabled, should extract clean content
    let config_with_extraction = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .md_readability(true)
        .build()
        .unwrap();

    assert_eq!(config_with_extraction.md_readability, true);
}
