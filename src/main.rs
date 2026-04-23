//! Crawlery - A flexible web crawler CLI.

use anyhow::Result;
use clap::Parser;
use crawlery::{CrawlConfig, CrawlMode, Crawler};
use std::path::PathBuf;

/// Crawlery - A flexible web crawler with HTTP and browser automation support
#[derive(Parser, Debug)]
#[command(name = "crawlery")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The starting URL to crawl (required if no recipe or pipeline is provided)
    #[arg(value_name = "URL")]
    url: Option<String>,

    /// Recipe file (YAML config) - contains all crawl settings
    #[arg(short = 'r', long, value_name = "FILE")]
    recipe: Option<PathBuf>,

    /// Run a multi-stage pipeline defined in a YAML file (stages separated by ---)
    #[arg(long, value_name = "FILE")]
    pipeline: Option<PathBuf>,

    /// Override the recipe's input_from path at runtime
    #[arg(long, value_name = "FILE")]
    input: Option<PathBuf>,

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
    let args = Args::parse();

    init_logging(args.verbose);

    // Handle pipeline mode first
    if let Some(pipeline_path) = &args.pipeline {
        return run_pipeline(pipeline_path, &args).await;
    }

    // Determine configuration source
    let mut config = if let Some(recipe_path) = &args.recipe {
        if args.verbose {
            println!("Loading recipe from: {}", recipe_path.display());
        }
        CrawlConfig::from_file(recipe_path)?
    } else if let Some(url) = &args.url {
        build_config_from_args(&args, url)?
    } else {
        anyhow::bail!(
            "Either a URL, a recipe file (--recipe), or a pipeline (--pipeline) must be provided.\n             Use --help for more information."
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

    let crawler = Crawler::new(config);

    let start_display = if crawler.config().url.is_empty() {
        crawler
            .config()
            .input_from
            .as_ref()
            .map(|p| format!("input_from: {}", p.display()))
            .unwrap_or_else(|| "(no url)".to_string())
    } else {
        crawler.config().url.clone()
    };
    println!("Starting crawl of {}...", start_display);

    match crawler.crawl().await {
        Ok(results) => {
            println!("\nCrawl completed successfully!");
            println!("Pages crawled: {}", results.len());

            save_crawl_results(&crawler, &results)?;

            Ok(())
        }
        Err(e) => {
            eprintln!("\nCrawl failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Save crawl results, applying extract_fields projection if specified.
fn save_crawl_results(crawler: &Crawler, results: &[crawlery::CrawlResult]) -> Result<()> {
    if !crawler.config().extract_fields.is_empty() {
        let projected = crawlery::transformers::project_fields(
            results,
            &crawler.config().extract_fields,
        );
        crawlery::output::save_projected(&projected, crawler.config().output_path.clone())?;
    } else {
        crawlery::output::save_results(
            results,
            crawler.config().output_format,
            crawler.config().output_path.clone(),
        )?;
    }
    Ok(())
}

/// Run a multi-stage pipeline.
async fn run_pipeline(path: &PathBuf, args: &Args) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read pipeline file '{}': {}", path.display(), e))?;

    let stages = CrawlConfig::parse_pipeline(&content)?;
    let stage_count = stages.len();

    if args.verbose {
        println!("Pipeline: {} stage(s)", stage_count);
    }

    for (i, mut config) in stages.into_iter().enumerate() {
        if let Some(output_path) = &args.output {
            config.output_path = Some(output_path.clone());
        }
        if let Some(input_path) = &args.input {
            config.input_from = Some(input_path.clone());
        }

        let stage_name = config
            .name
            .clone()
            .unwrap_or_else(|| format!("stage-{}", i + 1));

        let stage_info = if config.input_from.is_some() {
            format!("(input_from: {})", config.input_from.as_ref().unwrap().display())
        } else {
            format!("(url: {})", config.url)
        };

        println!(
            "\nPipeline stage {}/{}: {} {}",
            i + 1,
            stage_count,
            stage_name,
            stage_info
        );

        let crawler = Crawler::new(config);

        match crawler.crawl().await {
            Ok(results) => {
                println!("  Crawled {} pages", results.len());
                save_crawl_results(&crawler, &results)?;
            }
            Err(e) => {
                eprintln!("  Stage '{}' failed: {}", stage_name, e);
                return Err(e);
            }
        }
    }

    println!("\nPipeline complete.");
    Ok(())
}

fn build_config_from_args(_args: &Args, url: &str) -> Result<CrawlConfig> {
    CrawlConfig::builder()
        .url(url)
        .mode(CrawlMode::Http)
        .max_depth(2)
        .max_pages(50)
        .build()
}

fn apply_cli_overrides(config: &mut CrawlConfig, args: &Args) -> Result<()> {
    if let Some(output_path) = &args.output {
        config.output_path = Some(output_path.clone());
    }
    if let Some(input_path) = &args.input {
        config.input_from = Some(input_path.clone());
    }
    Ok(())
}

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

    let previous_state = crawlery::state::CrawlState::load(state_file)?;

    if verbose {
        println!("Loaded previous state:");
        println!("  Visited URLs: {}", previous_state.visited_count());
        println!("  Pending URLs: {}", previous_state.pending_count());
        println!("  Results: {}", previous_state.result_count());
    }

    let mut previous_results = Vec::new();
    if let Some(output_path) = &config.output_path {
        if output_path.exists() {
            if verbose {
                println!("Loading previous results from: {}", output_path.display());
            }
            previous_results = load_previous_results(output_path)?;
        }
    }

    let url_display = if config.url.is_empty() {
        config
            .input_from
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(no url)".to_string())
    } else {
        config.url.clone()
    };
    println!("Resuming crawl of {}...", url_display);
    println!("Already crawled: {} pages", previous_state.visited_count());
    println!("Remaining in queue: {} URLs", previous_state.pending_count());

    let crawler = Crawler::new(config);

    match crawler.crawl().await {
        Ok(mut results) => {
            println!("\nCrawl resumed successfully!");
            println!("New pages crawled: {}", results.len());

            if !previous_results.is_empty() {
                if verbose {
                    println!("Merging with {} previous results", previous_results.len());
                }
                previous_results.append(&mut results);
                results = previous_results;
            }

            println!("Total pages: {}", results.len());
            save_crawl_results(&crawler, &results)?;
            Ok(())
        }
        Err(e) => {
            eprintln!("\nCrawl failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn load_previous_results(path: &PathBuf) -> Result<Vec<crawlery::CrawlResult>> {
    let content = std::fs::read_to_string(path)?;

    if let Ok(results) = serde_json::from_str::<Vec<crawlery::CrawlResult>>(&content) {
        return Ok(results);
    }

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

fn print_config_summary(config: &CrawlConfig) {
    println!("Configuration:");
    if !config.url.is_empty() {
        println!("  URL: {}", config.url);
    }
    if let Some(input_from) = &config.input_from {
        println!("  Input from: {}", input_from.display());
    }
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
