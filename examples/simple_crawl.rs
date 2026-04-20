//! Simple example demonstrating basic Crawlery usage.
//!
//! This example shows how to:
//! - Create a crawler configuration
//! - Initialize a crawler
//! - Execute a crawl
//! - Process the results
//!
//! Run with:
//! ```bash
//! cargo run --example simple_crawl
//! ```

use crawlery::{CrawlConfig, CrawlMode, Crawler};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Crawlery Simple Example ===\n");

    // Create a configuration using the builder pattern
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(1)
        .max_concurrent_requests(5)
        .timeout_secs(30)
        .follow_redirects(true)
        .respect_robots_txt(true)
        .build()?;

    println!("Configuration:");
    println!("  URL: {}", config.url);
    println!("  Mode: {}", config.mode);
    println!("  Max Depth: {}", config.max_depth);
    println!();

    // Create the crawler
    let crawler = Crawler::new(config);

    // Start crawling
    println!("Starting crawl...\n");

    match crawler.crawl().await {
        Ok(results) => {
            println!("✓ Crawl completed successfully!\n");
            println!("Results Summary:");
            println!("  Total pages crawled: {}", results.len());
            println!();

            // Display detailed information about each result
            for (i, result) in results.iter().enumerate() {
                println!("Page {}:", i + 1);
                println!("  URL: {}", result.url);
                println!("  Status: {:?}", result.status_code);
                println!("  Title: {:?}", result.title);
                println!("  Links found: {}", result.link_count());
                println!("  Depth: {}", result.depth);
                println!("  Timestamp: {:?}", result.timestamp);

                if result.is_success() {
                    println!("  ✓ Successfully crawled");
                } else {
                    println!("  ✗ Failed or non-success status");
                }

                if !result.errors.is_empty() {
                    println!("  Errors:");
                    for error in &result.errors {
                        println!("    - {}", error);
                    }
                }

                println!();
            }

            // Display all unique links found
            let all_links: Vec<_> = results.iter().flat_map(|r| r.links.iter()).collect();

            if !all_links.is_empty() {
                println!("All discovered links:");
                for (i, link) in all_links.iter().take(10).enumerate() {
                    println!("  {}. {}", i + 1, link);
                }
                if all_links.len() > 10 {
                    println!("  ... and {} more", all_links.len() - 10);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Crawl failed: {}", e);
            eprintln!("\nError details: {:?}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
