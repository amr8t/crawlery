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

```bash
# Install
cargo install --path .

# Quick crawl with defaults (depth=2, max_pages=50)
crawlery https://example.com

# Use a recipe file (recommended for full control)
crawlery --recipe examples/recipes/basic_crawl.yaml

# Resume an interrupted crawl
crawlery --recipe my_recipe.yaml --resume

# Override output path
crawlery --recipe my_recipe.yaml -o custom_output.json
```

### CLI Options

```
Usage: crawlery [OPTIONS] [URL]

Options:
  -r, --recipe <FILE>  Recipe file (YAML config)
      --resume         Resume from existing state
  -o, --output <PATH>  Override output path
  -v, --verbose        Verbose output
  -h, --help           Show help
```

For advanced configuration, use recipe files (see `examples/recipes/`).

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

Recipe files define all crawl settings in YAML format for reproducible crawls.

### Example Recipe

```yaml
url: "https://example.com"
mode: http
max_depth: 3
max_pages: 100
output_path: "results.json"
output_format: json-pretty
state_file: "crawl.state"

include_patterns:
  - "^https://example\\.com/docs/"
exclude_patterns:
  - "\\.pdf$"
```

See `examples/recipes/` for templates:
- `basic_crawl.yaml` - General purpose
- `documentation_crawl.yaml` - Docs with URL filtering

### Create Recipe from Code

```rust
let config = CrawlConfig::builder()
    .url("https://example.com")
    .max_depth(3)
    .build()?;

config.to_file("my_recipe.yaml")?;
```

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

## License

AGPL-3.0