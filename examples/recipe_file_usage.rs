//! Example demonstrating recipe file usage for Crawlery.
//!
//! This example shows how to:
//! 1. Create a configuration and save it as a recipe file
//! 2. Load a configuration from a recipe file
//! 3. Use the recipe for crawling
//! 4. Resume a crawl from a saved state
//!
//! Run with: cargo run --example recipe_file_usage

use anyhow::Result;
use crawlery::{CrawlConfig, CrawlMode, Crawler, OutputFormat};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Crawlery Recipe File Usage Example ===\n");

    // Example 1: Create and save a recipe
    example_create_recipe()?;

    // Example 2: Load and use a recipe
    example_load_recipe().await?;

    // Example 3: Create a resumable crawl recipe
    example_resumable_recipe()?;

    println!("\n=== All examples completed ===");
    Ok(())
}

/// Example 1: Create a configuration and save it as a recipe file
fn example_create_recipe() -> Result<()> {
    println!("--- Example 1: Creating a Recipe File ---");

    // Build a configuration programmatically
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(3)
        .max_pages(100)
        .output_path(PathBuf::from("example_output.json"))
        .output_format(OutputFormat::JsonPretty)
        .state_file(PathBuf::from("example_state.json"))
        .timeout_secs(30)
        .max_concurrent_requests(10)
        .delay_ms(100)
        .max_retries(3)
        .follow_redirects(true)
        .respect_robots_txt(true)
        .include_pattern(r"^https://example\.com/.*")
        .exclude_pattern(r"\.pdf$")
        .exclude_pattern(r"\.zip$")
        .css_selector("article")
        .css_selector("main")
        .header("User-Agent".to_string(), "Crawlery Example/1.0".to_string())
        .build()?;

    // Save the configuration as a recipe file
    let recipe_path = "example_recipe.yaml";
    config.to_file(recipe_path)?;

    println!("✓ Created recipe file: {}", recipe_path);
    println!("✓ Configuration saved with all settings");
    println!();

    Ok(())
}

/// Example 2: Load a configuration from a recipe file and use it
async fn example_load_recipe() -> Result<()> {
    println!("--- Example 2: Loading and Using a Recipe ---");

    // Load configuration from the recipe file we just created
    let config = CrawlConfig::from_file("example_recipe.yaml")?;

    println!("✓ Loaded recipe from file");
    println!("  URL: {}", config.url);
    println!("  Mode: {}", config.mode);
    println!("  Max Depth: {}", config.max_depth);
    println!("  Output Format: {}", config.output_format);

    // Create a crawler with the loaded configuration
    let _crawler = Crawler::new(config);

    println!("\n✓ Crawler created with recipe configuration");
    println!("  (In a real scenario, you would call crawler.crawl().await)");
    println!();

    Ok(())
}

/// Example 3: Create a recipe for resumable crawling
fn example_resumable_recipe() -> Result<()> {
    println!("--- Example 3: Creating a Resumable Crawl Recipe ---");

    // Create a configuration optimized for resumable crawling
    let config = CrawlConfig::builder()
        .url("https://docs.example.com")
        .mode(CrawlMode::Http)
        .max_depth(5)
        .max_pages(1000) // Large crawl that might need to be resumed
        .output_path(PathBuf::from("docs_output.json"))
        .output_format(OutputFormat::Markdown)
        // State file is critical for resumable crawls
        .state_file(PathBuf::from("docs_state.json"))
        .timeout_secs(60)
        .max_concurrent_requests(5) // Lower concurrency for stability
        .delay_ms(500) // Polite delay
        .max_retries(5)
        .follow_redirects(true)
        .respect_robots_txt(true)
        .include_pattern(r"^https://docs\.example\.com/.*")
        .exclude_pattern(r"\.pdf$")
        .exclude_pattern(r"/login")
        .exclude_pattern(r"/logout")
        .build()?;

    // Save the resumable recipe
    let recipe_path = "resumable_example_recipe.yaml";
    config.to_file(recipe_path)?;

    println!("✓ Created resumable recipe: {}", recipe_path);
    println!("✓ State file configured: docs_state.json");
    println!();
    println!("To use this recipe:");
    println!("  1. Start crawl:  crawlery --recipe {} -v", recipe_path);
    println!("  2. Stop anytime:  Ctrl+C");
    println!(
        "  3. Resume:        crawlery --recipe {} --resume -v",
        recipe_path
    );
    println!();

    Ok(())
}

/// Example 4: Demonstrating recipe customization
#[allow(dead_code)]
fn example_customize_recipe() -> Result<()> {
    println!("--- Example 4: Customizing Recipes for Different Scenarios ---");

    // Scenario 1: Fast crawl for your own site
    let fast_config = CrawlConfig::builder()
        .url("https://mysite.com")
        .mode(CrawlMode::Http)
        .max_depth(5)
        .max_concurrent_requests(20) // High concurrency
        .delay_ms(0) // No delay for your own site
        .build()?;

    fast_config.to_file("fast_crawl_recipe.yaml")?;
    println!("✓ Created fast crawl recipe (for your own sites)");

    // Scenario 2: Polite crawl for external documentation
    let polite_config = CrawlConfig::builder()
        .url("https://external-docs.com")
        .mode(CrawlMode::Http)
        .max_depth(4)
        .max_concurrent_requests(3) // Low concurrency
        .delay_ms(1000) // 1 second delay between requests
        .respect_robots_txt(true)
        .build()?;

    polite_config.to_file("polite_crawl_recipe.yaml")?;
    println!("✓ Created polite crawl recipe (for external sites)");

    // Scenario 3: Browser mode for JavaScript-heavy sites
    let browser_config = CrawlConfig::builder()
        .url("https://spa-app.com")
        .mode(CrawlMode::Browser)
        .max_depth(2)
        .max_pages(50)
        .max_concurrent_requests(2) // Very low for browser mode
        .delay_ms(2000) // Longer delay for browser rendering
        .timeout_secs(90) // Longer timeout for JavaScript execution
        .build()?;

    browser_config.to_file("browser_crawl_recipe.yaml")?;
    println!("✓ Created browser mode recipe (for SPAs)");

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_load_recipe() -> Result<()> {
        // Create a test recipe
        let config = CrawlConfig::builder()
            .url("https://test.example.com")
            .max_depth(2)
            .build()?;

        let test_recipe = "test_recipe.yaml";
        config.to_file(test_recipe)?;

        // Load it back
        let loaded_config = CrawlConfig::from_file(test_recipe)?;

        // Verify the URL matches
        assert_eq!(loaded_config.url, "https://test.example.com");
        assert_eq!(loaded_config.max_depth, 2);

        // Cleanup
        std::fs::remove_file(test_recipe)?;

        Ok(())
    }

    #[test]
    fn test_recipe_with_patterns() -> Result<()> {
        // Create a recipe with include/exclude patterns
        let config = CrawlConfig::builder()
            .url("https://test.example.com")
            .include_pattern(r"^https://test\.example\.com/docs/.*")
            .exclude_pattern(r"\.pdf$")
            .build()?;

        let test_recipe = "test_patterns_recipe.yaml";
        config.to_file(test_recipe)?;

        // Load and verify patterns
        let loaded_config = CrawlConfig::from_file(test_recipe)?;

        assert_eq!(loaded_config.include_patterns.len(), 1);
        assert_eq!(loaded_config.exclude_patterns.len(), 1);
        assert_eq!(
            loaded_config.include_patterns[0],
            r"^https://test\.example\.com/docs/.*"
        );
        assert_eq!(loaded_config.exclude_patterns[0], r"\.pdf$");

        // Cleanup
        std::fs::remove_file(test_recipe)?;

        Ok(())
    }
}
