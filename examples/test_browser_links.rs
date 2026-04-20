//! Test browser link extraction to verify the fix.
//!
//! This example tests the browser crawler's link extraction functionality
//! with the netboxlabs.com URL that was previously returning 0 links.
//!
//! Run with: cargo run --example test_browser_links

use anyhow::Result;
use crawlery::browser::{BrowserConfig, BrowserCrawler};

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  Browser Link Extraction Test                                   ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    let test_url = "https://netboxlabs.com/docs/netbox/";

    println!("Testing URL: {}\n", test_url);
    println!("─────────────────────────────────────────────────────────────────\n");

    // Create browser crawler
    let config = BrowserConfig {
        proxy: None,
        user_agent: Some("CrawleryTest/1.0".to_string()),
        timeout_secs: 30,
        headless: true,
    };

    println!("Creating browser crawler with config:");
    println!("  User Agent: {:?}", config.user_agent);
    println!("  Timeout: {} seconds", config.timeout_secs);
    println!("  Headless: {}\n", config.headless);

    let crawler = BrowserCrawler::new(config)?;

    println!("─────────────────────────────────────────────────────────────────\n");
    println!("Starting browser crawl...\n");
    println!("─────────────────────────────────────────────────────────────────\n");

    // Fetch the page
    let result = crawler.fetch(test_url)?;

    println!("\n─────────────────────────────────────────────────────────────────");
    println!("RESULTS");
    println!("─────────────────────────────────────────────────────────────────\n");

    println!("✓ URL: {}", result.url);
    println!("✓ HTML Length: {} bytes", result.html.len());
    println!("✓ Content Length: {} bytes", result.cleaned_content.len());
    println!("✓ Status Code: {:?}", result.status_code);
    println!("\n✓ LINKS FOUND: {}\n", result.links.len());

    if result.links.is_empty() {
        println!("❌ ERROR: No links found! The bug still exists.\n");
        println!("Expected: 100+ links");
        println!("Got: 0 links");
        return Err(anyhow::anyhow!("Link extraction failed"));
    } else {
        println!("✅ SUCCESS: Links extracted successfully!\n");

        println!("First 20 links:");
        for (i, link) in result.links.iter().take(20).enumerate() {
            println!("  {}: {}", i + 1, link);
        }

        if result.links.len() > 20 {
            println!("  ... and {} more links", result.links.len() - 20);
        }
    }

    println!("\n─────────────────────────────────────────────────────────────────");
    println!("Content Preview (first 500 chars):");
    println!("─────────────────────────────────────────────────────────────────\n");

    let preview = if result.cleaned_content.len() > 500 {
        format!("{}...", &result.cleaned_content[..500])
    } else {
        result.cleaned_content.clone()
    };
    println!("{}", preview);

    println!("\n─────────────────────────────────────────────────────────────────");
    println!("SUMMARY");
    println!("─────────────────────────────────────────────────────────────────\n");

    if result.links.len() >= 100 {
        println!(
            "✅ PASS: Found {} links (expected 100+)",
            result.links.len()
        );
        println!("✅ Browser link extraction is working correctly!");
    } else if result.links.len() > 0 {
        println!(
            "⚠️  PARTIAL: Found {} links (expected 100+)",
            result.links.len()
        );
        println!("   Links are being extracted, but fewer than expected.");
    } else {
        println!("❌ FAIL: Found 0 links");
        println!("   The link extraction bug persists.");
    }

    println!();
    Ok(())
}
