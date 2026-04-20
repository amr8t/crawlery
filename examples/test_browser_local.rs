//! Test browser link extraction with a local HTML file.
//!
//! This example tests the browser crawler's link extraction functionality
//! using a controlled local HTML file with known link counts.
//!
//! Run with: cargo run --example test_browser_local

use anyhow::Result;
use crawlery::browser::{BrowserConfig, BrowserCrawler};
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  Browser Link Extraction - Local HTML Test                     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Get the path to the test HTML file
    let mut html_path = PathBuf::from(env::current_dir()?);
    html_path.push("test_page.html");

    if !html_path.exists() {
        eprintln!("❌ Error: test_page.html not found at {:?}", html_path);
        eprintln!("\nPlease ensure test_page.html exists in the project root.");
        return Err(anyhow::anyhow!("Test file not found"));
    }

    let file_url = format!("file://{}", html_path.display());
    println!("Test file: {}", html_path.display());
    println!("File URL: {}\n", file_url);

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
    let result = crawler.fetch(&file_url)?;

    println!("\n─────────────────────────────────────────────────────────────────");
    println!("RESULTS");
    println!("─────────────────────────────────────────────────────────────────\n");

    println!("✓ URL: {}", result.url);
    println!("✓ HTML Length: {} bytes", result.html.len());
    println!("✓ Content Length: {} bytes", result.cleaned_content.len());
    println!("✓ Status Code: {:?}", result.status_code);
    println!("\n✓ LINKS FOUND: {}\n", result.links.len());

    // Expected: ~30+ valid HTTP/HTTPS links from our test page
    let expected_min_links = 30;

    if result.links.is_empty() {
        println!("❌ ERROR: No links found! The bug still exists.\n");
        println!("Expected: {}+ links", expected_min_links);
        println!("Got: 0 links");
        return Err(anyhow::anyhow!("Link extraction failed - no links found"));
    } else {
        println!("✅ SUCCESS: Links extracted successfully!\n");

        println!("All {} extracted links:", result.links.len());
        for (i, link) in result.links.iter().enumerate() {
            println!("  {}: {}", i + 1, link);
        }
    }

    println!("\n─────────────────────────────────────────────────────────────────");
    println!("Link Analysis:");
    println!("─────────────────────────────────────────────────────────────────\n");

    let https_count = result
        .links
        .iter()
        .filter(|l| l.starts_with("https://"))
        .count();
    let http_count = result
        .links
        .iter()
        .filter(|l| l.starts_with("http://"))
        .count();
    let other_count = result.links.len() - https_count - http_count;

    println!("  HTTPS links: {}", https_count);
    println!("  HTTP links:  {}", http_count);
    println!("  Other:       {}", other_count);
    println!("  Total:       {}", result.links.len());

    // Check for specific expected links
    println!("\n─────────────────────────────────────────────────────────────────");
    println!("Validation Checks:");
    println!("─────────────────────────────────────────────────────────────────\n");

    let has_github = result.links.iter().any(|l| l.contains("github.com"));
    let has_example_com = result.links.iter().any(|l| l.contains("example.com"));
    let has_google = result.links.iter().any(|l| l.contains("google.com"));
    let has_rust = result.links.iter().any(|l| l.contains("rust-lang.org"));

    println!(
        "  ✓ Contains github.com: {}",
        if has_github { "YES ✅" } else { "NO ❌" }
    );
    println!(
        "  ✓ Contains example.com: {}",
        if has_example_com { "YES ✅" } else { "NO ❌" }
    );
    println!(
        "  ✓ Contains google.com: {}",
        if has_google { "YES ✅" } else { "NO ❌" }
    );
    println!(
        "  ✓ Contains rust-lang.org: {}",
        if has_rust { "YES ✅" } else { "NO ❌" }
    );

    // Check that invalid links are filtered out
    let has_javascript = result.links.iter().any(|l| l.starts_with("javascript:"));
    let has_mailto = result.links.iter().any(|l| l.starts_with("mailto:"));
    let has_tel = result.links.iter().any(|l| l.starts_with("tel:"));

    println!(
        "\n  ✓ Filtered javascript:: {}",
        if !has_javascript { "YES ✅" } else { "NO ❌" }
    );
    println!(
        "  ✓ Filtered mailto:: {}",
        if !has_mailto { "YES ✅" } else { "NO ❌" }
    );
    println!(
        "  ✓ Filtered tel:: {}",
        if !has_tel { "YES ✅" } else { "NO ❌" }
    );

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
    println!("FINAL SUMMARY");
    println!("─────────────────────────────────────────────────────────────────\n");

    let all_checks_passed = has_github
        && has_example_com
        && has_google
        && has_rust
        && !has_javascript
        && !has_mailto
        && !has_tel;

    if result.links.len() >= expected_min_links && all_checks_passed {
        println!(
            "✅ ALL TESTS PASSED: Found {} links (expected {}+)",
            result.links.len(),
            expected_min_links
        );
        println!("✅ All validation checks passed!");
        println!("✅ Browser link extraction is working correctly!");
    } else if result.links.len() >= expected_min_links {
        println!(
            "⚠️  PARTIAL PASS: Found {} links (expected {}+)",
            result.links.len(),
            expected_min_links
        );
        println!("⚠️  Some validation checks failed - review results above.");
    } else if result.links.len() > 0 {
        println!(
            "⚠️  PARTIAL: Found {} links (expected {}+)",
            result.links.len(),
            expected_min_links
        );
        println!("   Links are being extracted, but fewer than expected.");
        println!("   This may indicate the page didn't fully load or JS didn't execute.");
    } else {
        println!("❌ FAIL: Found 0 links");
        println!("   The link extraction bug persists.");
    }

    println!();
    Ok(())
}
