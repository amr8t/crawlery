# Crawlery

A fast, RAG-optimized web crawler built in Rust. Extracts clean content from web pages using readability-style algorithms, perfect for LLM/RAG applications.

## Features

- 🚀 Dual crawling modes: HTTP (fast) & headless Chrome (JavaScript support)
- 🧠 Intelligent content extraction: Removes navigation, ads, footers while preserving main content
- 📝 Clean Markdown output: Optimized for LLM ingestion with proper structure
- 🏷️ Metadata extraction: Title, author, date, description from OpenGraph/Schema.org
- 🔄 Resumable crawling: State persistence for interrupted crawls with automatic resume
- 📋 Recipe files: YAML-based configuration for reproducible crawls
- 🎯 Auto-discovery: Configurable depth and intelligent link following
- 🔍 URL filtering: Regex include/exclude patterns
- 🌐 Proxy rotation: Multi-proxy support
- 📊 Multiple formats: JSON, Markdown, CSV, Text

## Quick Start

### CLI Usage

```bash
# Install
cargo install --path .

# Basic crawl with CLI arguments
crawlery https://example.com -m http -d 2 -o results.json

# Use a recipe file (recommended)
crawlery --recipe examples/recipes/basic_crawl.yaml -v

# Resume an interrupted crawl
crawlery --recipe examples/recipes/resumable_crawl.yaml --resume
```

### Library Usage

```bash
cargo add crawlery
```

## Library Example

```rust
use crawlery::{CrawlConfig, CrawlMode, Crawler};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .mode(CrawlMode::Http)
        .max_depth(2)
        .build()?;

    let crawler = Crawler::new(config);
    let results = crawler.crawl().await?;
    println!("Crawled {} pages", results.len());
    Ok(())
}
```

## Content Extraction for RAG

Crawlery uses readability algorithms to extract clean content perfect for RAG pipelines:

```rust
use crawlery::content::extract_content;

let html = "<article><h1>Title</h1><p>Content</p></article>";
let markdown = extract_content(html)?;  // Returns: "# Title\n\nContent\n"
```

**Removes**: Navigation, ads, footers, sidebars, scripts, cookie banners  
**Preserves**: Main content, headings, lists, quotes, document structure

## Recipe Files

Recipe files are YAML configuration files that define all crawl settings in one place, making crawls reproducible and easy to share.

### Creating a Recipe

```yaml
url: "https://example.com"
mode: http
max_depth: 3
max_pages: 100
output_path: "results.json"
output_format: json-pretty
state_file: "crawl.state"  # Enable resumable crawling
timeout_secs: 30
delay_ms: 500
respect_robots_txt: true

include_patterns:
  - "^https://example\\.com/docs/.*"
exclude_patterns:
  - "\\.pdf$"
  - "/login"
```

### Using Recipes

```bash
# Run a crawl with a recipe
crawlery --recipe my_recipe.yaml -v

# Override output path
crawlery --recipe my_recipe.yaml --output custom_output.json

# Create a recipe programmatically
```

```rust
use crawlery::CrawlConfig;

let config = CrawlConfig::builder()
    .url("https://example.com")
    .max_depth(3)
    .build()?;

config.to_file("my_recipe.yaml")?;
```

See `examples/recipes/` for ready-to-use recipe templates:
- `basic_crawl.yaml` - Simple general-purpose crawl
- `documentation_crawl.yaml` - Optimized for docs with URL filtering
- `resumable_crawl.yaml` - Long-running resumable crawl
- `browser_crawl.yaml` - JavaScript-heavy sites

## Resumable Crawling

Pause and resume long-running crawls without losing progress.

### Setup

Configure a state file in your recipe:

```yaml
state_file: "my_crawl.state"
output_path: "results.json"
```

### Usage

```bash
# Start a crawl
crawlery --recipe my_recipe.yaml -v

# Stop anytime (Ctrl+C)
# The state is automatically saved

# Resume from where you left off
crawlery --recipe my_recipe.yaml --resume -v
```

### How It Works

When resuming:
1. Loads the state file with visited URLs and pending queue
2. Loads existing results from the output file
3. Continues crawling from the pending queue
4. Merges new results with previous results
5. Skips already-visited URLs

Perfect for:
- Large-scale crawls (1000+ pages)
- Unstable network connections
- Rate-limited sites requiring breaks
- Incremental documentation updates

## Documentation

Full API documentation: `cargo doc --open`

Recipe file documentation: See `examples/recipes/README.md`

## License

AGPL-3.0