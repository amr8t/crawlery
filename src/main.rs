//! Crawlery - A flexible web crawler CLI.
//!
//! This binary provides a command-line interface for the Crawlery web crawler library.

use anyhow::Result;
use clap::Parser;
use crawlery::{CrawlConfig, CrawlMode, Crawler};
use std::path::PathBuf;

/// Crawlery - A flexible web crawler with HTTP and browser automation support
#[derive(Parser, Debug)]
#[command(name = "crawlery")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The starting URL to crawl (required if no recipe is provided)
    #[arg(value_name = "URL")]
    url: Option<String>,

    /// Recipe file (YAML config) - contains all crawl settings
    #[arg(short = 'r', long, value_name = "FILE")]
    recipe: Option<PathBuf>,

    /// Resume from existing output/state file
    #[arg(long)]
    resume: bool,

    /// Override output file path
    #[arg(short = 'o', long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short = 'v', long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize logging based on verbosity
    init_logging(args.verbose);

    // Determine configuration source
    let mut config = if let Some(recipe_path) = &args.recipe {
        // Load configuration from recipe file
        if args.verbose {
            println!("Loading recipe from: {}", recipe_path.display());
        }
        CrawlConfig::from_file(recipe_path)?
    } else if let Some(url) = &args.url {
        // Build configuration from CLI arguments (backward compatibility)
        build_config_from_args(&args, url)?
    } else {
        anyhow::bail!(
            "Either a URL or a recipe file (--recipe) must be provided.\n\
             Use --help for more information."
        );
    };

    // Apply CLI overrides to recipe config
    if args.recipe.is_some() {
        apply_cli_overrides(&mut config, &args)?;
    }

    // Handle resume mode
    if args.resume {
        if args.verbose {
            println!("Resume mode enabled");
        }
        return resume_crawl(config, args.verbose).await;
    }

    if args.verbose {
        print_config_summary(&config);
    }

    // Create crawler and start crawling
    let crawler = Crawler::new(config);

    println!("Starting crawl of {}...", crawler.config().url);

    match crawler.crawl().await {
        Ok(results) => {
            println!("\nCrawl completed successfully!");
            println!("Pages crawled: {}", results.len());

            // Save or print results in the specified format
            crawlery::output::save_results(
                &results,
                crawler.config().output_format,
                crawler.config().output_path.clone(),
            )?;

            // Save state if state_file is configured
            if let Some(state_file) = &crawler.config().state_file {
                if args.verbose {
                    println!("Saving state to: {}", state_file.display());
                }
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("\nCrawl failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Build configuration from CLI arguments (legacy mode)
/// Build minimal configuration from CLI arguments (use recipes for full control)
fn build_config_from_args(_args: &Args, url: &str) -> Result<CrawlConfig> {
    // Simple quick-crawl config - use recipe files for advanced options
    CrawlConfig::builder()
        .url(url)
        .mode(CrawlMode::Http)
        .max_depth(2)
        .max_pages(50)
        .build()
}

/// Apply CLI argument overrides to recipe configuration
fn apply_cli_overrides(config: &mut CrawlConfig, args: &Args) -> Result<()> {
    // Output path override
    if let Some(output_path) = &args.output {
        config.output_path = Some(output_path.clone());
    }

    Ok(())
}

/// Resume crawl from existing state file
async fn resume_crawl(config: CrawlConfig, verbose: bool) -> Result<()> {
    let state_file = config
        .state_file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No state_file configured for resume mode"))?;

    if !state_file.exists() {
        anyhow::bail!(
            "State file not found: {}. Cannot resume.",
            state_file.display()
        );
    }

    if verbose {
        println!("Loading state from: {}", state_file.display());
    }

    // Load previous state
    let previous_state = crawlery::state::CrawlState::load(state_file)?;

    if verbose {
        println!("Loaded previous state:");
        println!("  Visited URLs: {}", previous_state.visited_count());
        println!("  Pending URLs: {}", previous_state.pending_count());
        println!("  Results: {}", previous_state.result_count());
    }

    // Load previous results if output file exists
    let mut previous_results = Vec::new();
    if let Some(output_path) = &config.output_path {
        if output_path.exists() {
            if verbose {
                println!("Loading previous results from: {}", output_path.display());
            }
            previous_results = load_previous_results(output_path)?;
            if verbose {
                println!("  Previous results: {}", previous_results.len());
            }
        }
    }

    // Create crawler with existing state
    println!("Resuming crawl of {}...", config.url);
    println!("Already crawled: {} pages", previous_state.visited_count());
    println!(
        "Remaining in queue: {} URLs",
        previous_state.pending_count()
    );

    let crawler = Crawler::new(config);

    match crawler.crawl().await {
        Ok(mut results) => {
            println!("\nCrawl resumed successfully!");
            println!("New pages crawled: {}", results.len());

            // Merge with previous results
            if !previous_results.is_empty() {
                if verbose {
                    println!("Merging with {} previous results", previous_results.len());
                }
                previous_results.append(&mut results);
                results = previous_results;
            }

            println!("Total pages: {}", results.len());

            // Save merged results
            crawlery::output::save_results(
                &results,
                crawler.config().output_format,
                crawler.config().output_path.clone(),
            )?;

            Ok(())
        }
        Err(e) => {
            eprintln!("\nCrawl failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Load previous results from output file
fn load_previous_results(path: &PathBuf) -> Result<Vec<crawlery::CrawlResult>> {
    let content = std::fs::read_to_string(path)?;

    // Try to parse as JSON array
    if let Ok(results) = serde_json::from_str::<Vec<crawlery::CrawlResult>>(&content) {
        return Ok(results);
    }

    // If not an array, might be newline-delimited JSON
    let results: Result<Vec<_>> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<crawlery::CrawlResult>(line)
                .map_err(|e| anyhow::anyhow!("Failed to parse result line: {}", e))
        })
        .collect();

    results
}

/// Print configuration summary
fn print_config_summary(config: &CrawlConfig) {
    println!("Configuration:");
    println!("  URL: {}", config.url);
    println!("  Mode: {}", config.mode);
    println!("  Max Depth: {}", config.max_depth);
    if let Some(max_pages) = config.max_pages {
        println!("  Max Pages: {}", max_pages);
    }
    println!("  Output Format: {}", config.output_format);
    if let Some(output_path) = &config.output_path {
        println!("  Output Path: {}", output_path.display());
    }
    if let Some(state_file) = &config.state_file {
        println!("  State File: {}", state_file.display());
    }
}

/// Initialize logging based on verbosity level.
fn init_logging(verbose: bool) {
    if verbose {
        println!("Verbose logging enabled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
