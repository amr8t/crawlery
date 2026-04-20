//! Crawlery - A flexible web crawler CLI.
//!
//! This binary provides a command-line interface for the Crawlery web crawler library.

use anyhow::Result;
use clap::Parser;
use crawlery::{CrawlConfig, CrawlMode, Crawler, OutputFormat};
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
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    // Legacy CLI options for backward compatibility (when no recipe is used)
    /// Crawl mode: http or browser
    #[arg(short = 'm', long, value_name = "MODE")]
    mode: Option<String>,

    /// Maximum depth to crawl (0 = only start URL)
    #[arg(short = 'd', long, value_name = "DEPTH")]
    max_depth: Option<usize>,

    /// Maximum number of pages to crawl
    #[arg(short = 'p', long, value_name = "COUNT")]
    max_pages: Option<usize>,

    /// Output format: json, json-pretty, markdown, csv, text
    #[arg(short = 'f', long, value_name = "FORMAT")]
    format: Option<String>,

    /// State file path for resumable crawling
    #[arg(short = 's', long, value_name = "FILE")]
    state_file: Option<PathBuf>,

    /// Proxy URL (e.g., http://proxy.example.com:8080)
    #[arg(long, value_name = "URL")]
    proxy: Option<String>,

    /// User agent string
    #[arg(short = 'u', long, value_name = "STRING")]
    user_agent: Option<String>,

    /// Request timeout in seconds
    #[arg(short = 't', long, value_name = "SECONDS")]
    timeout: Option<u64>,

    /// Maximum number of concurrent requests
    #[arg(short = 'c', long, value_name = "COUNT")]
    concurrent: Option<usize>,

    /// Delay between requests in milliseconds
    #[arg(long, value_name = "MS")]
    delay: Option<u64>,

    /// Maximum number of retries per request
    #[arg(long, value_name = "COUNT")]
    retries: Option<usize>,

    /// Don't follow redirects
    #[arg(long)]
    no_redirects: bool,

    /// Don't respect robots.txt
    #[arg(long)]
    ignore_robots: bool,

    /// URL patterns to include (regex, can be specified multiple times)
    #[arg(short = 'i', long = "include", value_name = "PATTERN")]
    include_patterns: Vec<String>,

    /// URL patterns to exclude (regex, can be specified multiple times)
    #[arg(short = 'e', long = "exclude", value_name = "PATTERN")]
    exclude_patterns: Vec<String>,

    /// CSS selectors to extract (can be specified multiple times)
    #[arg(long = "selector", value_name = "SELECTOR")]
    selectors: Vec<String>,

    /// Additional HTTP headers in format "Key: Value" (can be specified multiple times)
    #[arg(short = 'H', long = "header", value_name = "HEADER")]
    headers: Vec<String>,
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
fn build_config_from_args(args: &Args, url: &str) -> Result<CrawlConfig> {
    // Parse mode
    let mode: CrawlMode = if let Some(mode_str) = &args.mode {
        mode_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid mode '{}': {}", mode_str, e))?
    } else {
        CrawlMode::Http
    };

    // Parse output format
    let output_format: OutputFormat = if let Some(format_str) = &args.format {
        format_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid format '{}': {}", format_str, e))?
    } else {
        OutputFormat::Json
    };

    // Parse headers
    let mut headers = std::collections::HashMap::new();
    for header in &args.headers {
        if let Some((key, value)) = header.split_once(':') {
            headers.insert(key.trim().to_string(), value.trim().to_string());
        } else {
            anyhow::bail!("Invalid header format '{}'. Expected 'Key: Value'", header);
        }
    }

    // Build configuration
    let mut builder = CrawlConfig::builder()
        .url(url)
        .mode(mode)
        .max_depth(args.max_depth.unwrap_or(3))
        .output_format(output_format)
        .timeout_secs(args.timeout.unwrap_or(30))
        .max_concurrent_requests(args.concurrent.unwrap_or(10))
        .delay_ms(args.delay.unwrap_or(0))
        .max_retries(args.retries.unwrap_or(3))
        .follow_redirects(!args.no_redirects)
        .respect_robots_txt(!args.ignore_robots);

    // Add optional settings
    if let Some(max_pages) = args.max_pages {
        builder = builder.max_pages(max_pages);
    }

    if let Some(output_path) = &args.output {
        builder = builder.output_path(output_path.clone());
    }

    if let Some(state_file) = &args.state_file {
        builder = builder.state_file(state_file.clone());
    }

    if let Some(user_agent) = &args.user_agent {
        builder = builder.user_agent(user_agent);
    }

    if let Some(proxy_url) = &args.proxy {
        builder = builder.proxy(crawlery::ProxyConfig::new(proxy_url));
    }

    // Add patterns
    for pattern in &args.include_patterns {
        builder = builder.include_pattern(pattern);
    }

    for pattern in &args.exclude_patterns {
        builder = builder.exclude_pattern(pattern);
    }

    // Add CSS selectors
    for selector in &args.selectors {
        builder = builder.css_selector(selector);
    }

    // Add custom headers
    for (key, value) in headers {
        builder = builder.header(key, value);
    }

    // Build and validate configuration
    builder.build()
}

/// Apply CLI argument overrides to recipe configuration
fn apply_cli_overrides(config: &mut CrawlConfig, args: &Args) -> Result<()> {
    // Output path override
    if let Some(output_path) = &args.output {
        config.output_path = Some(output_path.clone());
    }

    // State file override
    if let Some(state_file) = &args.state_file {
        config.state_file = Some(state_file.clone());
    }

    // Verbose mode doesn't need to override config

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
    println!("Starting crawl with configuration:");
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
    if !config.include_patterns.is_empty() {
        println!("  Include Patterns: {:?}", config.include_patterns);
    }
    if !config.exclude_patterns.is_empty() {
        println!("  Exclude Patterns: {:?}", config.exclude_patterns);
    }
    println!("  Timeout: {}s", config.timeout_secs);
    println!("  Delay: {}ms", config.delay_ms);
    println!("  Concurrent Requests: {}", config.max_concurrent_requests);
    println!("  Respect robots.txt: {}", config.respect_robots_txt);
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
