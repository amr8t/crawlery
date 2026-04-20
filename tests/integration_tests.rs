//! Integration tests for Crawlery core functionality.
//!
//! These tests verify that the core types, builders, and APIs work correctly
//! when used together as they would be in a real application.

use crawlery::{
    CrawlConfig, CrawlError, CrawlMode, CrawlResult, Crawler, OutputFormat, ProxyConfig,
};

use std::path::PathBuf;

// ============================================================================
// CrawlConfig Tests
// ============================================================================

#[test]
fn test_crawl_config_builder_minimal() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .build()
        .expect("Failed to build config");

    assert_eq!(config.url, "https://example.com");
    assert_eq!(config.mode, CrawlMode::Http);
    assert_eq!(config.max_depth, 3);
    assert_eq!(config.output_format, OutputFormat::Json);
    assert_eq!(config.timeout_secs, 30);
    assert_eq!(config.max_concurrent_requests, 10);
    assert_eq!(config.delay_ms, 0);
    assert_eq!(config.max_retries, 3);
    assert!(config.follow_redirects);
    assert!(config.respect_robots_txt);
}

#[test]
fn test_crawl_config_builder_full() {
    let proxy = ProxyConfig::new("http://proxy.example.com:8080");

    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Browser)
        .max_depth(5)
        .output_path("/tmp/output.json")
        .output_format(OutputFormat::Markdown)
        .proxy(proxy)
        .user_agent("TestBot/1.0")
        .timeout_secs(60)
        .max_concurrent_requests(20)
        .delay_ms(500)
        .max_retries(5)
        .follow_redirects(false)
        .respect_robots_txt(false)
        .include_pattern(r"^https://example\.com/blog/")
        .exclude_pattern(r"\.pdf$")
        .css_selector("article")
        .header("Accept", "application/json")
        .build()
        .expect("Failed to build config");

    assert_eq!(config.url, "https://example.com");
    assert_eq!(config.mode, CrawlMode::Browser);
    assert_eq!(config.max_depth, 5);
    assert_eq!(config.output_path, Some(PathBuf::from("/tmp/output.json")));
    assert_eq!(config.output_format, OutputFormat::Markdown);
    assert!(config.proxy.is_some());
    assert_eq!(config.user_agent, Some("TestBot/1.0".to_string()));
    assert_eq!(config.timeout_secs, 60);
    assert_eq!(config.max_concurrent_requests, 20);
    assert_eq!(config.delay_ms, 500);
    assert_eq!(config.max_retries, 5);
    assert!(!config.follow_redirects);
    assert!(!config.respect_robots_txt);
    assert_eq!(config.include_patterns.len(), 1);
    assert_eq!(config.exclude_patterns.len(), 1);
    assert_eq!(config.css_selectors.len(), 1);
    assert_eq!(config.headers.len(), 1);
}

#[test]
fn test_crawl_config_builder_missing_url() {
    let result = CrawlConfig::builder().build();

    assert!(result.is_err());
}

#[test]
fn test_crawl_config_validation_invalid_url() {
    let result = CrawlConfig::builder().url("not a valid url").build();

    assert!(result.is_err());
}

#[test]
fn test_crawl_config_validation_invalid_regex_include() {
    let result = CrawlConfig::builder()
        .url("https://example.com")
        .include_pattern("[invalid(regex")
        .build();

    assert!(result.is_err());
}

#[test]
fn test_crawl_config_validation_invalid_regex_exclude() {
    let result = CrawlConfig::builder()
        .url("https://example.com")
        .exclude_pattern("[invalid(regex")
        .build();

    assert!(result.is_err());
}

#[test]
fn test_crawl_config_multiple_patterns() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .include_pattern(r"^https://example\.com/blog/")
        .include_pattern(r"^https://example\.com/docs/")
        .exclude_pattern(r"\.pdf$")
        .exclude_pattern(r"\.jpg$")
        .exclude_pattern(r"/admin/")
        .build()
        .expect("Failed to build config");

    assert_eq!(config.include_patterns.len(), 2);
    assert_eq!(config.exclude_patterns.len(), 3);
}

#[test]
fn test_crawl_config_multiple_selectors() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .css_selector("article")
        .css_selector("div.content")
        .css_selector("main")
        .build()
        .expect("Failed to build config");

    assert_eq!(config.css_selectors.len(), 3);
}

#[test]
fn test_crawl_config_multiple_headers() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .header("Accept", "application/json")
        .header("Authorization", "Bearer token")
        .header("X-Custom", "value")
        .build()
        .expect("Failed to build config");

    assert_eq!(config.headers.len(), 3);
    assert_eq!(
        config.headers.get("Accept"),
        Some(&"application/json".to_string())
    );
    assert_eq!(
        config.headers.get("Authorization"),
        Some(&"Bearer token".to_string())
    );
    assert_eq!(config.headers.get("X-Custom"), Some(&"value".to_string()));
}

// ============================================================================
// CrawlMode Tests
// ============================================================================

#[test]
fn test_crawl_mode_from_str_http() {
    let mode: CrawlMode = "http".parse().expect("Failed to parse mode");
    assert_eq!(mode, CrawlMode::Http);
}

#[test]
fn test_crawl_mode_from_str_browser() {
    let mode: CrawlMode = "browser".parse().expect("Failed to parse mode");
    assert_eq!(mode, CrawlMode::Browser);
}

#[test]
fn test_crawl_mode_from_str_case_insensitive() {
    assert_eq!("HTTP".parse::<CrawlMode>().unwrap(), CrawlMode::Http);
    assert_eq!("BROWSER".parse::<CrawlMode>().unwrap(), CrawlMode::Browser);
    assert_eq!("Http".parse::<CrawlMode>().unwrap(), CrawlMode::Http);
    assert_eq!("Browser".parse::<CrawlMode>().unwrap(), CrawlMode::Browser);
}

#[test]
fn test_crawl_mode_from_str_invalid() {
    let result = "invalid".parse::<CrawlMode>();
    assert!(result.is_err());
}

#[test]
fn test_crawl_mode_display() {
    assert_eq!(CrawlMode::Http.to_string(), "http");
    assert_eq!(CrawlMode::Browser.to_string(), "browser");
}

// ============================================================================
// OutputFormat Tests
// ============================================================================

#[test]
fn test_output_format_from_str() {
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!(
        "json-pretty".parse::<OutputFormat>().unwrap(),
        OutputFormat::JsonPretty
    );
    assert_eq!(
        "markdown".parse::<OutputFormat>().unwrap(),
        OutputFormat::Markdown
    );
    assert_eq!(
        "md".parse::<OutputFormat>().unwrap(),
        OutputFormat::Markdown
    );
    assert_eq!("csv".parse::<OutputFormat>().unwrap(), OutputFormat::Csv);
    assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
    assert_eq!("txt".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
}

#[test]
fn test_output_format_from_str_case_insensitive() {
    assert_eq!("JSON".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!(
        "MARKDOWN".parse::<OutputFormat>().unwrap(),
        OutputFormat::Markdown
    );
    assert_eq!(
        "MD".parse::<OutputFormat>().unwrap(),
        OutputFormat::Markdown
    );
}

#[test]
fn test_output_format_from_str_invalid() {
    let result = "invalid".parse::<OutputFormat>();
    assert!(result.is_err());
}

#[test]
fn test_output_format_display() {
    assert_eq!(OutputFormat::Json.to_string(), "json");
    assert_eq!(OutputFormat::JsonPretty.to_string(), "json-pretty");
    assert_eq!(OutputFormat::Markdown.to_string(), "markdown");
    assert_eq!(OutputFormat::Csv.to_string(), "csv");
    assert_eq!(OutputFormat::Text.to_string(), "text");
}

// ============================================================================
// ProxyConfig Tests
// ============================================================================

#[test]
fn test_proxy_config_new() {
    let proxy = ProxyConfig::new("http://proxy.example.com:8080");

    assert_eq!(proxy.url, "http://proxy.example.com:8080");
    assert_eq!(proxy.username, None);
    assert_eq!(proxy.password, None);
}

#[test]
fn test_proxy_config_with_auth() {
    let proxy = ProxyConfig::new("http://proxy.example.com:8080").with_auth("user", "pass");

    assert_eq!(proxy.url, "http://proxy.example.com:8080");
    assert_eq!(proxy.username, Some("user".to_string()));
    assert_eq!(proxy.password, Some("pass".to_string()));
}

// ============================================================================
// CrawlResult Tests
// ============================================================================

#[test]
fn test_crawl_result_new() {
    let result = CrawlResult::new(
        "https://example.com".to_string(),
        "<html></html>".to_string(),
        0,
    );

    assert_eq!(result.url, "https://example.com");
    assert_eq!(result.content, "<html></html>");
    assert_eq!(result.depth, 0);
    assert_eq!(result.status_code, None);
    assert_eq!(result.title, None);
    assert_eq!(result.links.len(), 0);
    assert_eq!(result.metadata.len(), 0);
    assert_eq!(result.errors.len(), 0);
}

#[test]
fn test_crawl_result_link_count() {
    let mut result = CrawlResult::new(
        "https://example.com".to_string(),
        "<html></html>".to_string(),
        0,
    );

    assert_eq!(result.link_count(), 0);

    result.links.push("https://example.com/page1".to_string());
    result.links.push("https://example.com/page2".to_string());

    assert_eq!(result.link_count(), 2);
}

#[test]
fn test_crawl_result_is_success() {
    let mut result = CrawlResult::new("https://example.com".to_string(), "content".to_string(), 0);

    // No status code
    assert!(!result.is_success());

    // Success codes (2xx)
    result.status_code = Some(200);
    assert!(result.is_success());

    result.status_code = Some(201);
    assert!(result.is_success());

    result.status_code = Some(299);
    assert!(result.is_success());

    // Failure codes
    result.status_code = Some(404);
    assert!(!result.is_success());

    result.status_code = Some(500);
    assert!(!result.is_success());

    result.status_code = Some(301);
    assert!(!result.is_success());
}

#[test]
fn test_crawl_result_with_metadata() {
    let mut result = CrawlResult::new("https://example.com".to_string(), "content".to_string(), 0);

    result
        .metadata
        .insert("author".to_string(), "John Doe".to_string());
    result
        .metadata
        .insert("date".to_string(), "2024-01-01".to_string());

    assert_eq!(result.metadata.len(), 2);
    assert_eq!(result.metadata.get("author"), Some(&"John Doe".to_string()));
    assert_eq!(result.metadata.get("date"), Some(&"2024-01-01".to_string()));
}

#[test]
fn test_crawl_result_with_headers() {
    let mut result = CrawlResult::new("https://example.com".to_string(), "content".to_string(), 0);

    result
        .headers
        .insert("content-type".to_string(), "text/html".to_string());
    result
        .headers
        .insert("server".to_string(), "nginx".to_string());

    assert_eq!(result.headers.len(), 2);
    assert_eq!(
        result.headers.get("content-type"),
        Some(&"text/html".to_string())
    );
}

// ============================================================================
// Crawler Tests
// ============================================================================

#[test]
fn test_crawler_new() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .build()
        .expect("Failed to build config");

    let crawler = Crawler::new(config);
    assert_eq!(crawler.config().url, "https://example.com");
}

#[test]
fn test_crawler_config_reference() {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Browser)
        .max_depth(5)
        .build()
        .expect("Failed to build config");

    let crawler = Crawler::new(config);
    let crawler_config = crawler.config();

    assert_eq!(crawler_config.url, "https://example.com");
    assert_eq!(crawler_config.mode, CrawlMode::Browser);
    assert_eq!(crawler_config.max_depth, 5);
}

// ============================================================================
// Error Tests
// ============================================================================

#[test]
fn test_crawl_error_http_error_display() {
    let err = CrawlError::HttpError {
        url: "https://example.com".to_string(),
        message: "404 Not Found".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("https://example.com"));
    assert!(display.contains("404 Not Found"));
}

#[test]
fn test_crawl_error_invalid_url_display() {
    let err = CrawlError::InvalidUrl {
        url: "not-a-url".to_string(),
        reason: "missing scheme".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("not-a-url"));
    assert!(display.contains("missing scheme"));
}

#[test]
fn test_crawl_error_timeout_display() {
    let err = CrawlError::Timeout {
        url: "https://slow.example.com".to_string(),
        duration_secs: 30,
    };

    let display = format!("{}", err);
    assert!(display.contains("https://slow.example.com"));
    assert!(display.contains("30"));
}

#[test]
fn test_crawl_error_max_depth_exceeded_display() {
    let err = CrawlError::MaxDepthExceeded {
        url: "https://example.com/deep/page".to_string(),
        max_depth: 5,
    };

    let display = format!("{}", err);
    assert!(display.contains("https://example.com/deep/page"));
    assert!(display.contains("5"));
}

#[test]
fn test_crawl_error_validation_error_display() {
    let err = CrawlError::ValidationError {
        field: "max_depth".to_string(),
        message: "must be positive".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("max_depth"));
    assert!(display.contains("must be positive"));
}

// ============================================================================
// Integration Scenarios
// ============================================================================

#[test]
fn test_builder_chaining_complex_config() {
    // Test that builder methods can be chained in any order
    let config = CrawlConfig::builder()
        .include_pattern(r"^https://example\.com/blog/")
        .url("https://example.com")
        .exclude_pattern(r"\.pdf$")
        .mode(CrawlMode::Http)
        .css_selector("article")
        .max_depth(3)
        .header("Accept", "text/html")
        .timeout_secs(45)
        .user_agent("TestBot")
        .build()
        .expect("Failed to build config");

    assert_eq!(config.url, "https://example.com");
    assert_eq!(config.mode, CrawlMode::Http);
    assert_eq!(config.max_depth, 3);
    assert_eq!(config.include_patterns.len(), 1);
    assert_eq!(config.exclude_patterns.len(), 1);
    assert_eq!(config.css_selectors.len(), 1);
    assert_eq!(config.headers.len(), 1);
}

#[test]
fn test_url_schemes() {
    // Test various valid URL schemes
    let urls = vec![
        "https://example.com",
        "http://example.com",
        "https://example.com:8080",
        "https://example.com/path",
        "https://example.com/path?query=value",
        "https://example.com/path#fragment",
        "https://user:pass@example.com",
    ];

    for url in urls {
        let result = CrawlConfig::builder().url(url).build();
        assert!(result.is_ok(), "Failed to parse valid URL: {}", url);
    }
}

#[test]
fn test_invalid_url_schemes() {
    // Test various invalid URLs that should be rejected by the URL parser
    let invalid_urls = vec!["not-a-url", "", "ht!tp://invalid"];

    for url in invalid_urls {
        let result = CrawlConfig::builder().url(url).build();
        assert!(result.is_err(), "Should reject invalid URL: {}", url);
    }

    // Note: These are technically valid URLs according to the URL standard,
    // even though they may not be suitable for web crawling:
    // - "ftp://example.com" (valid FTP URL)
    // - "javascript:alert(1)" (valid javascript: URL)
    // - "file:///etc/passwd" (valid file: URL)
    // Additional scheme validation could be added in the future if needed.
}

#[test]
fn test_serialization_deserialization() {
    // Test that types can be serialized and deserialized
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Browser)
        .max_depth(2)
        .output_format(OutputFormat::Markdown)
        .build()
        .expect("Failed to build config");

    // Serialize to JSON
    let json = serde_json::to_string(&config).expect("Failed to serialize");

    // Deserialize back
    let deserialized: CrawlConfig = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(config.url, deserialized.url);
    assert_eq!(config.mode, deserialized.mode);
    assert_eq!(config.max_depth, deserialized.max_depth);
}

#[test]
fn test_crawl_result_serialization() {
    let mut result = CrawlResult::new(
        "https://example.com".to_string(),
        "<html></html>".to_string(),
        0,
    );

    result.status_code = Some(200);
    result.title = Some("Example".to_string());
    result.links.push("https://example.com/page1".to_string());

    // Serialize to JSON
    let json = serde_json::to_string(&result).expect("Failed to serialize");

    // Deserialize back
    let deserialized: CrawlResult = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(result.url, deserialized.url);
    assert_eq!(result.status_code, deserialized.status_code);
    assert_eq!(result.title, deserialized.title);
    assert_eq!(result.links, deserialized.links);
}
