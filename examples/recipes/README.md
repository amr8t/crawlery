# Crawlery Recipe Files

This directory contains example recipe files for Crawlery. Recipe files are YAML configuration files that define all crawl settings in a single, reusable file.

## What are Recipe Files?

Recipe files are YAML-based configuration files that contain all the settings needed for a web crawl, including:

- Target URL and crawl mode
- Depth and page limits
- Output format and file paths
- URL filtering patterns
- Request settings (timeouts, delays, concurrency)
- State file configuration for resumable crawls
- Custom headers and user agents
- Proxy settings

## Benefits

- **Reproducibility**: Save and share exact crawl configurations
- **Version Control**: Track changes to crawl settings over time
- **Simplicity**: No need to remember complex CLI arguments
- **Documentation**: Recipe files self-document the crawl purpose
- **Resumability**: Easy to set up resumable crawls with state files

## Usage

### Basic Usage

Run a crawl using a recipe file:

```bash
crawlery --recipe examples/recipes/basic_crawl.yaml
```

With verbose output:

```bash
crawlery --recipe examples/recipes/basic_crawl.yaml -v
```

### Override Output Path

Override the output path specified in the recipe:

```bash
crawlery --recipe examples/recipes/basic_crawl.yaml --output custom_output.json
```

### Resume a Crawl

Resume a previously interrupted crawl:

```bash
crawlery --recipe examples/recipes/resumable_crawl.yaml --resume
```

The `--resume` flag will:
1. Load the state file specified in the recipe
2. Skip already-visited URLs
3. Continue from the pending queue
4. Merge new results with existing results

## Example Recipes

### `basic_crawl.yaml`

A simple starter recipe demonstrating basic crawling configuration.

**Use case**: General-purpose web crawling with reasonable defaults.

```bash
crawlery --recipe examples/recipes/basic_crawl.yaml
```

### `documentation_crawl.yaml`

Optimized for crawling documentation websites with extensive URL filtering.

**Use case**: Indexing documentation sites while excluding non-content pages (login, download, external links, etc.).

**Features**:
- Include/exclude patterns for documentation paths
- CSS selectors for content extraction
- Markdown output format
- Respectful crawl settings (delays, lower concurrency)

```bash
crawlery --recipe examples/recipes/documentation_crawl.yaml
```

### `resumable_crawl.yaml`

Example configuration for long-running crawls that can be paused and resumed.

**Use case**: Large-scale crawls that may take hours or days, need to be paused, or might be interrupted.

**Features**:
- State file configuration
- Conservative request settings for stability
- High page limits
- Detailed comments on resume workflow

**Start the crawl**:
```bash
crawlery --recipe examples/recipes/resumable_crawl.yaml -v
```

**Resume after interruption**:
```bash
crawlery --recipe examples/recipes/resumable_crawl.yaml --resume -v
```

### `browser_crawl.yaml`

Example for crawling JavaScript-heavy sites using headless browser automation.

**Use case**: Single-page applications (SPAs), React/Vue/Angular sites, or any site requiring JavaScript execution.

**Features**:
- Browser mode configuration
- Lower concurrency (browser is resource-intensive)
- Higher timeouts for page rendering
- Longer delays between requests

```bash
crawlery --recipe examples/recipes/browser_crawl.yaml
```

## Creating Custom Recipes

### 1. Start with a Template

Copy one of the example recipes and modify it:

```bash
cp examples/recipes/basic_crawl.yaml my_custom_recipe.yaml
```

### 2. Edit the Configuration

Open the file and modify the settings:

```yaml
url: "https://your-site.com"
mode: http  # or browser
max_depth: 3
max_pages: 100
output_path: "results.json"
output_format: json-pretty
state_file: "crawl.state"  # for resumable crawls
# ... other settings
```

### 3. Test Your Recipe

Run with verbose output to verify settings:

```bash
crawlery --recipe my_custom_recipe.yaml -v
```

### 4. Save the Recipe

Save your recipe to the recipes directory:

```bash
mv my_custom_recipe.yaml examples/recipes/
```

## Recipe File Format

### Required Fields

```yaml
url: "https://example.com"          # Starting URL
mode: http                           # http or browser
max_depth: 3                         # Crawl depth
output_format: json-pretty           # Output format
```

### Optional Fields

```yaml
max_pages: 100                       # Limit total pages (null for unlimited)
output_path: "output.json"           # Save results to file
state_file: "crawl.state"            # Enable resumable crawling
timeout_secs: 30                     # Request timeout
max_concurrent_requests: 10          # Concurrent requests
delay_ms: 0                          # Delay between requests
max_retries: 3                       # Retry attempts
follow_redirects: true               # Follow HTTP redirects
respect_robots_txt: true             # Honor robots.txt
user_agent: "Crawlery/1.0"           # Custom user agent
```

### URL Filtering

Use regex patterns to include or exclude URLs:

```yaml
include_patterns:
  - "^https://example\\.com/docs/.*"
  - "^https://example\\.com/api/.*"

exclude_patterns:
  - "\\.pdf$"
  - "\\.zip$"
  - "/login"
  - "/logout"
```

### Content Extraction

Extract specific elements using CSS selectors:

```yaml
css_selectors:
  - "article"
  - "main"
  - ".content"
```

### Custom Headers

Add custom HTTP headers:

```yaml
headers:
  User-Agent: "Crawlery/1.0"
  Accept-Language: "en-US,en;q=0.9"
  Authorization: "Bearer token"
```

### Proxy Configuration

Configure proxy settings:

```yaml
proxy:
  url: "http://proxy.example.com:8080"
  username: "user"
  password: "pass"
```

## Resumable Crawling

Resumable crawling allows you to pause and resume long-running crawls.

### Setup

1. **Configure state file** in your recipe:

```yaml
state_file: "my_crawl.state"
```

2. **Start the crawl**:

```bash
crawlery --recipe my_recipe.yaml -v
```

The crawler saves state periodically to the state file.

3. **Stop the crawl** (Ctrl+C) at any time.

4. **Resume the crawl**:

```bash
crawlery --recipe my_recipe.yaml --resume -v
```

### How Resume Works

When resuming:

1. **Loads state file**: Reads visited URLs, pending queue, and configuration
2. **Loads previous results**: If output file exists, loads existing results
3. **Continues crawling**: Processes remaining URLs in the pending queue
4. **Merges results**: Combines new results with previous results
5. **Saves updated state**: Updates state file as crawling progresses

### Best Practices for Resumable Crawls

- **Use conservative settings**: Lower concurrency, reasonable delays
- **Set appropriate timeouts**: Avoid hanging requests
- **Monitor state file size**: Can grow large for massive crawls
- **Back up state files**: For very long crawls, periodically back up
- **Use verbose mode**: Monitor progress with `-v` flag
- **Test first**: Test on a small subset before large crawls

## Tips and Best Practices

### Crawl Speed vs. Politeness

- **Fast crawls**: Higher concurrency, lower delays (may get rate-limited)
- **Polite crawls**: Lower concurrency (3-5), delays of 500-1000ms

```yaml
# Fast (for your own sites)
max_concurrent_requests: 20
delay_ms: 0

# Polite (for external sites)
max_concurrent_requests: 5
delay_ms: 500
```

### Browser vs. HTTP Mode

- **Use HTTP mode** for:
  - Static websites
  - Server-rendered content
  - Fast crawling
  - Large-scale crawls

- **Use Browser mode** for:
  - JavaScript-heavy sites
  - Single-page applications (SPAs)
  - Sites requiring user interaction simulation
  - Content loaded via AJAX

### URL Filtering Strategies

1. **Whitelist approach** (recommended for documentation):
   - Use `include_patterns` to only crawl specific paths
   - Prevents wandering to unrelated sections

2. **Blacklist approach**:
   - Use `exclude_patterns` to skip known non-content URLs
   - More permissive but may crawl unwanted pages

3. **Combined approach**:
   - Use both include and exclude patterns
   - Most control but requires careful configuration

### Output Formats

Choose based on your use case:

- **json**: Compact, machine-readable
- **json-pretty**: Human-readable, larger file size
- **markdown**: Great for documentation, readable
- **csv**: Easy to import into spreadsheets
- **text**: Simple text output, minimal formatting

## Troubleshooting

### Recipe File Not Found

```
Error: No such file or directory
```

**Solution**: Check the path to your recipe file. Use absolute or relative paths.

### Invalid YAML Syntax

```
Failed to parse YAML recipe file
```

**Solution**: Validate your YAML syntax. Common issues:
- Incorrect indentation (use spaces, not tabs)
- Missing colons
- Unquoted special characters in strings

### State File Not Found (Resume)

```
State file not found: crawl.state. Cannot resume.
```

**Solution**: 
- Ensure you've run the crawl at least once to create the state file
- Check the `state_file` path in your recipe
- Verify the file exists before using `--resume`

### URL Pattern Errors

```
Invalid regex 'pattern': ...
```

**Solution**: Test your regex patterns. Remember to escape special characters:
- Use `\\.` for literal dots
- Use `\\$` for end of string
- Use `.*` for any characters

## Advanced Usage

### Programmatic Recipe Generation

Create recipes programmatically:

```rust
use crawlery::CrawlConfig;

let config = CrawlConfig::builder()
    .url("https://example.com")
    .max_depth(3)
    .build()?;

config.to_file("generated_recipe.yaml")?;
```

### Loading Recipes in Code

Load and use recipes in your Rust code:

```rust
use crawlery::{CrawlConfig, Crawler};

let config = CrawlConfig::from_file("recipe.yaml")?;
let crawler = Crawler::new(config);
let results = crawler.crawl().await?;
```

### Batch Processing

Process multiple recipes:

```bash
for recipe in examples/recipes/*.yaml; do
    echo "Running $recipe..."
    crawlery --recipe "$recipe" -v
done
```

## Contributing

Have a useful recipe? Contribute it to the repository:

1. Create your recipe file
2. Test it thoroughly
3. Add documentation in comments
4. Submit a pull request

## License

All example recipes are provided under the same license as Crawlery (MIT).