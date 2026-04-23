# Crawlery

A fast, efficient and customizable web crawler built in Rust. Extracts clean content from web pages using readability-style algorithms, perfect for LLM/RAG applications.

## Features

- Dual crawling modes: HTTP (fast) & headless Chrome (JavaScript support)
- Optional content extraction: Enable readability-style cleaning for RAG/LLM applications
- Metadata extraction: Title, author, date, description from OpenGraph/Schema.org
- Resumable crawling: State persistence for interrupted crawls with automatic resume
- Recipe files: YAML-based configuration for reproducible crawls
- Auto-discovery: Configurable depth and intelligent link following
- URL filtering: Regex include/exclude patterns

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

## Content Extraction Options

Crawlery provides two output modes:

### 1. Raw HTML (Default)
By default, Crawlery returns the raw HTML content, just like traditional scrapers. This gives you the complete page structure to process yourself.

### 2. Clean Content Extraction (Optional - for RAG/LLM)
Enable `extract_content: true` to use readability algorithms that extract clean content perfect for RAG pipelines:

```rust
use crawlery::{CrawlConfig, CrawlMode, Crawler};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CrawlConfig::builder()
        .url("https://example.com")
        .extract_content(true)  // Enable clean content extraction
        .build()?;

    let crawler = Crawler::new(config);
    let results = crawler.crawl().await?;
    // Results contain cleaned markdown-like content
    Ok(())
}
```

Or in a recipe file:
```yaml
url: "https://example.com"
extract_content: true  # Enable for RAG/LLM applications
```

**When enabled, removes**: Navigation, ads, footers, sidebars, scripts, cookie banners  
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

# Content extraction (optional)
extract_content: false  # false = raw HTML (default), true = clean content for RAG/LLM

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


## Code-Based Pipeline Composition

Use the `Pipeline` builder to compose multi-stage crawls in Rust -- conditional execution,
fork-join splitting, and runtime config overrides -- without touching any YAML.

### Quick Example

```rust
use crawlery::pipeline::Pipeline;

let results = Pipeline::new()
    .stage("discover", "recipes/discover.yaml").end()
    .stage("extract", "recipes/extract.yaml")
        .when(|prev| !prev.is_empty())        // skip if nothing found
    .end()
    .run().await?;
```

Each stage's results are forwarded as URL inputs to the next stage automatically.
Use `.transform()` to reshape results in-process before they are forwarded.

## Documentation

Full API documentation: `cargo doc --open`

## License

AGPL-3.0
