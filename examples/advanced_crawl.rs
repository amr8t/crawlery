//! Advanced example demonstrating Crawlery's filtering and configuration features.
//!
//! This example shows how to:
//! - Use URL pattern filtering (include/exclude)
//! - Extract specific content with CSS selectors
//! - Configure custom headers and user agent
//! - Set up rate limiting and concurrency
//! - Use proxy configuration
//! - Handle different output formats
//!
//! Run with:
//! ```bash
//! cargo run --example advanced_crawl
//! ```

use crawlery::{CrawlConfig, CrawlMode, Crawler, OutputFormat, ProxyConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Crawlery Advanced Example ===\n");

    // Example 1: Crawl with URL filtering
    println!("Example 1: URL Pattern Filtering");
    println!("---------------------------------");
    let config1 = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(2)
        // Only include blog posts
        .include_pattern(r"^https://example\.com/blog/")
        // Exclude PDF files and admin pages
        .exclude_pattern(r"\.pdf$")
        .exclude_pattern(r"/admin/")
        .build()?;

    print_config(&config1);

    // Example 2: Content extraction with CSS selectors
    println!("\nExample 2: CSS Selector Extraction");
    println!("-----------------------------------");
    let config2 = CrawlConfig::builder()
        .url("https://news.ycombinator.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        // Extract specific elements
        .css_selector("article.title")
        .css_selector("div.content")
        .css_selector("span.score")
        .max_concurrent_requests(5)
        .build()?;

    print_config(&config2);

    // Example 3: Custom headers and authentication
    println!("\nExample 3: Custom Headers and User Agent");
    println!("----------------------------------------");
    let config3 = CrawlConfig::builder()
        .url("https://api.example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        // Set custom user agent
        .user_agent("MyCrawler/1.0 (Advanced Example)")
        // Add custom headers
        .header("Accept", "application/json")
        .header("X-Custom-Header", "CustomValue")
        .timeout_secs(60)
        .build()?;

    print_config(&config3);

    // Example 4: Rate limiting and politeness
    println!("\nExample 4: Rate Limiting");
    println!("------------------------");
    let config4 = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(3)
        // Limit concurrent requests to be polite
        .max_concurrent_requests(2)
        // Add delay between requests (1 second)
        .delay_ms(1000)
        // Set reasonable timeout
        .timeout_secs(30)
        // Respect robots.txt
        .respect_robots_txt(true)
        .build()?;

    print_config(&config4);

    // Example 5: Proxy configuration
    println!("\nExample 5: Proxy Configuration");
    println!("------------------------------");

    // Create proxy with authentication
    let proxy = ProxyConfig::new("http://proxy.example.com:8080").with_auth("username", "password");

    let config5 = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .proxy(proxy)
        .build()?;

    print_config(&config5);

    // Example 6: Different output formats
    println!("\nExample 6: Output Formats");
    println!("-------------------------");

    let formats = vec![
        OutputFormat::Json,
        OutputFormat::JsonPretty,
        OutputFormat::Markdown,
        OutputFormat::Csv,
        OutputFormat::Text,
    ];

    for format in formats {
        let config = CrawlConfig::builder()
            .url("https://example.com")
            .mode(CrawlMode::Http)
            .max_depth(1)
            .output_format(format)
            .build()?;

        println!("  Format: {}", config.output_format);
    }

    // Example 7: Complex configuration combining multiple features
    println!("\nExample 7: Complex Configuration");
    println!("--------------------------------");
    let config7 = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(3)
        .output_format(OutputFormat::JsonPretty)
        // Network settings
        .timeout_secs(45)
        .max_concurrent_requests(8)
        .delay_ms(500)
        .max_retries(5)
        .follow_redirects(true)
        // Filtering
        .include_pattern(r"^https://example\.com/(blog|docs)/")
        .exclude_pattern(r"\.(jpg|png|gif|pdf)$")
        .exclude_pattern(r"/private/")
        // Content extraction
        .css_selector("article")
        .css_selector("main.content")
        // Custom headers
        .user_agent("AdvancedCrawler/1.0")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept-Encoding", "gzip, deflate, br")
        // Behavior
        .respect_robots_txt(true)
        .build()?;

    print_config(&config7);

    // Example 8: Demonstrate actual crawling with error handling
    println!("\nExample 8: Executing a Crawl");
    println!("----------------------------");

    let crawl_config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .max_concurrent_requests(3)
        .timeout_secs(30)
        .build()?;

    let crawler = Crawler::new(crawl_config);

    println!("Starting crawl of https://example.com...\n");

    match crawler.crawl().await {
        Ok(results) => {
            println!("✓ Crawl completed successfully!");
            println!("\nStatistics:");
            println!("  Total pages: {}", results.len());

            let successful = results.iter().filter(|r| r.is_success()).count();
            let failed = results.len() - successful;

            println!("  Successful: {}", successful);
            println!("  Failed: {}", failed);

            let total_links: usize = results.iter().map(|r| r.link_count()).sum();
            println!("  Total links found: {}", total_links);

            // Show depth distribution
            let mut depth_map: HashMap<usize, usize> = HashMap::new();
            for result in &results {
                *depth_map.entry(result.depth).or_insert(0) += 1;
            }

            println!("\nDepth distribution:");
            for depth in 0..=*depth_map.keys().max().unwrap_or(&0) {
                if let Some(count) = depth_map.get(&depth) {
                    println!("  Depth {}: {} pages", depth, count);
                }
            }

            // Show sample results
            if !results.is_empty() {
                println!("\nSample results:");
                for result in results.iter().take(3) {
                    println!("  - {}", result.url);
                    if let Some(title) = &result.title {
                        println!("    Title: {}", title);
                    }
                    println!("    Links: {}", result.link_count());
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Crawl failed: {}", e);
            eprintln!("\nThis is expected in this example as the crawler");
            eprintln!("implementation is not yet complete.");
        }
    }

    println!("\n=== Examples Complete ===");
    Ok(())
}

/// Helper function to print configuration details
fn print_config(config: &CrawlConfig) {
    println!("Configuration:");
    println!("  URL: {}", config.url);
    println!("  Mode: {}", config.mode);
    println!("  Max Depth: {}", config.max_depth);
    println!("  Timeout: {}s", config.timeout_secs);
    println!("  Concurrent Requests: {}", config.max_concurrent_requests);
    println!("  Delay: {}ms", config.delay_ms);
    println!("  Max Retries: {}", config.max_retries);
    println!("  Follow Redirects: {}", config.follow_redirects);
    println!("  Respect robots.txt: {}", config.respect_robots_txt);

    if let Some(user_agent) = &config.user_agent {
        println!("  User Agent: {}", user_agent);
    }

    if !config.include_patterns.is_empty() {
        println!("  Include Patterns:");
        for pattern in &config.include_patterns {
            println!("    - {}", pattern);
        }
    }

    if !config.exclude_patterns.is_empty() {
        println!("  Exclude Patterns:");
        for pattern in &config.exclude_patterns {
            println!("    - {}", pattern);
        }
    }

    if !config.css_selectors.is_empty() {
        println!("  CSS Selectors:");
        for selector in &config.css_selectors {
            println!("    - {}", selector);
        }
    }

    if !config.headers.is_empty() {
        println!("  Custom Headers:");
        for (key, value) in &config.headers {
            println!("    {}: {}", key, value);
        }
    }

    if config.proxy.is_some() {
        println!("  Proxy: Configured");
    }
}
