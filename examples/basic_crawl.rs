//! Basic crawl example demonstrating the integrated Crawlery crawler.
//!
//! This example shows how to:
//! - Configure a crawler with various options
//! - Perform HTTP-based crawling
//! - Handle results
//! - Use state files for resumable crawling
//!
//! Run with:
//!   cargo run --example basic_crawl

use crawlery::{CrawlConfig, CrawlMode, Crawler, OutputFormat};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Crawlery Basic Crawl Example ===\n");

    // Example 1: Simple HTTP crawl
    println!("Example 1: Simple HTTP crawl");
    simple_crawl().await?;

    println!("\n{}", "=".repeat(50));

    // Example 2: Crawl with state file for resumability
    println!("\nExample 2: Crawl with state file");
    crawl_with_state().await?;

    println!("\n{}", "=".repeat(50));

    // Example 3: Crawl with URL filtering
    println!("\nExample 3: Crawl with URL filtering");
    crawl_with_filters().await?;

    println!("\n{}", "=".repeat(50));

    // Example 4: Advanced configuration
    println!("\nExample 4: Advanced configuration");
    advanced_crawl().await?;

    Ok(())
}

/// Simple HTTP crawl with minimal configuration
async fn simple_crawl() -> anyhow::Result<()> {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .max_pages(5)
        .respect_robots_txt(false) // For testing only
        .timeout_secs(10)
        .build()?;

    let crawler = Crawler::new(config);
    println!("Starting crawl of example.com...");

    match crawler.crawl().await {
        Ok(results) => {
            println!("✓ Crawled {} pages successfully", results.len());
            for result in results.iter().take(3) {
                println!(
                    "  - {} (status: {}, links: {})",
                    result.url,
                    result.status_code.unwrap_or(0),
                    result.links.len()
                );
            }
        }
        Err(e) => {
            println!("✗ Crawl failed: {}", e);
        }
    }

    Ok(())
}

/// Crawl with state file for resumability
async fn crawl_with_state() -> anyhow::Result<()> {
    let state_file = PathBuf::from("/tmp/crawlery_state.json");

    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(2)
        .max_pages(10)
        .state_file(state_file.clone())
        .delay_ms(500)
        .respect_robots_txt(false)
        .timeout_secs(10)
        .build()?;

    let crawler = Crawler::new(config);
    println!("Starting crawl with state file: {:?}", state_file);
    println!("(Crawl can be resumed if interrupted)");

    match crawler.crawl().await {
        Ok(results) => {
            println!("✓ Crawled {} pages", results.len());
            println!("✓ State saved to: {:?}", state_file);

            // Show some statistics
            let successful = results.iter().filter(|r| r.is_success()).count();
            let total_links: usize = results.iter().map(|r| r.link_count()).sum();

            println!("\nStatistics:");
            println!("  - Successful pages: {}/{}", successful, results.len());
            println!("  - Total links found: {}", total_links);
        }
        Err(e) => {
            println!("✗ Crawl failed: {}", e);
        }
    }

    Ok(())
}

/// Crawl with URL filtering (include/exclude patterns)
async fn crawl_with_filters() -> anyhow::Result<()> {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(2)
        .max_pages(10)
        // Only crawl URLs under /docs/ path
        .include_pattern(r"^https://example\.com(/|/docs/.*)$")
        // Exclude PDF and ZIP files
        .exclude_pattern(r".*\.(pdf|zip)$")
        .respect_robots_txt(false)
        .timeout_secs(10)
        .build()?;

    let crawler = Crawler::new(config);
    println!("Starting crawl with URL filters...");
    println!("  Include: URLs under /docs/");
    println!("  Exclude: .pdf and .zip files");

    match crawler.crawl().await {
        Ok(results) => {
            println!("✓ Crawled {} filtered pages", results.len());
            for result in results.iter().take(5) {
                println!("  - {}", result.url);
            }
        }
        Err(e) => {
            println!("✗ Crawl failed: {}", e);
        }
    }

    Ok(())
}

/// Advanced crawl configuration with all options
async fn advanced_crawl() -> anyhow::Result<()> {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(2)
        .max_pages(20)
        .max_concurrent_requests(5)
        .delay_ms(1000) // 1 second delay between requests
        .timeout_secs(30)
        .max_retries(3)
        .follow_redirects(true)
        .respect_robots_txt(true)
        .user_agent("MyCrawler/1.0 (Custom Bot)")
        // Add custom headers
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept", "text/html,application/xhtml+xml")
        // Extract specific CSS selectors
        .css_selector("article")
        .css_selector("main")
        // Output configuration
        .output_format(OutputFormat::JsonPretty)
        .output_path("/tmp/crawl_results.json")
        .build()?;

    let crawler = Crawler::new(config);
    println!("Starting advanced crawl with custom configuration...");
    println!("Configuration:");
    println!("  - Max depth: {}", crawler.config().max_depth);
    println!("  - Max pages: {:?}", crawler.config().max_pages);
    println!(
        "  - Concurrent requests: {}",
        crawler.config().max_concurrent_requests
    );
    println!("  - Delay: {}ms", crawler.config().delay_ms);
    println!("  - User agent: {:?}", crawler.config().user_agent);

    match crawler.crawl().await {
        Ok(results) => {
            println!("\n✓ Crawl completed successfully!");
            println!("  Pages crawled: {}", results.len());

            // Analyze results
            let avg_links = if !results.is_empty() {
                results.iter().map(|r| r.link_count()).sum::<usize>() / results.len()
            } else {
                0
            };

            println!("\nAnalysis:");
            println!("  - Average links per page: {}", avg_links);
            println!(
                "  - Pages with titles: {}",
                results.iter().filter(|r| r.title.is_some()).count()
            );
            println!(
                "  - Successful pages: {}",
                results.iter().filter(|r| r.is_success()).count()
            );

            // Show depth distribution
            println!("\nDepth distribution:");
            for depth in 0..=2 {
                let count = results.iter().filter(|r| r.depth == depth).count();
                if count > 0 {
                    println!("  - Depth {}: {} pages", depth, count);
                }
            }
        }
        Err(e) => {
            println!("✗ Crawl failed: {}", e);
            println!("  Error details: {:?}", e);
        }
    }

    Ok(())
}
