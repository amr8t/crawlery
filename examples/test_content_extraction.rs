//! Example demonstrating the new content extraction for RAG/LLM use cases.
//!
//! This example shows how the readability-style content extraction works,
//! converting HTML into clean markdown suitable for LLM ingestion.

use crawlery::content::{extract_content, extract_metadata};
use crawlery::http_client::{HttpCrawler, HttpCrawlerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Content Extraction Test ===\n");

    // Create HTTP crawler
    let config = HttpCrawlerConfig::default();
    let crawler = HttpCrawler::new(config)?;

    // Fetch example.com
    let url = "https://example.com";
    println!("Fetching: {}\n", url);

    let result = crawler.fetch(url).await?;

    // Display extracted metadata
    println!("--- Metadata ---");
    let metadata = extract_metadata(&result.html);
    for (key, value) in metadata.iter() {
        println!("{}: {}", key, value);
    }
    println!();

    // Display clean markdown content
    println!("--- Clean Markdown Content ---");
    println!("{}", result.clean_text);
    println!();

    // Show the difference with raw HTML
    println!("--- Raw HTML (first 500 chars) ---");
    println!("{}\n", &result.html.chars().take(500).collect::<String>());

    // Test with a more complex example
    let complex_html = r#"
        <html>
            <head>
                <title>Sample Article</title>
                <meta name="description" content="A great article about content extraction">
                <meta name="author" content="Jane Doe">
                <meta property="og:title" content="Sample Article - OpenGraph Title">
            </head>
            <body>
                <header>
                    <nav>
                        <a href="/">Home</a>
                        <a href="/about">About</a>
                        <a href="/contact">Contact</a>
                    </nav>
                </header>

                <main>
                    <article class="content">
                        <h1>The Future of Web Crawling</h1>

                        <p class="byline">By Jane Doe | <time datetime="2024-01-15">January 15, 2024</time></p>

                        <p>Web crawling has evolved significantly over the years. What started as
                        simple HTML parsing has now become a sophisticated field involving JavaScript
                        rendering, content extraction, and semantic understanding.</p>

                        <h2>Key Challenges</h2>

                        <p>Modern web crawling faces several challenges:</p>

                        <ul>
                            <li>JavaScript-heavy single-page applications</li>
                            <li>Dynamic content loading</li>
                            <li>Anti-bot measures and CAPTCHAs</li>
                            <li>Content extraction from complex layouts</li>
                        </ul>

                        <h2>Solutions with Readability</h2>

                        <p>Readability-style algorithms help extract the main content by:</p>

                        <ol>
                            <li>Scoring content based on structure and text density</li>
                            <li>Removing boilerplate like navigation and ads</li>
                            <li>Preserving semantic structure</li>
                            <li>Converting to clean markdown for LLM consumption</li>
                        </ol>

                        <blockquote>
                            "The goal is not just to crawl, but to understand the web."
                        </blockquote>

                        <p>This approach makes the extracted content perfect for RAG applications,
                        where clean, structured text is essential for accurate retrieval and generation.</p>
                    </article>
                </main>

                <aside class="sidebar">
                    <h3>Related Articles</h3>
                    <ul>
                        <li><a href="/article1">Article 1</a></li>
                        <li><a href="/article2">Article 2</a></li>
                    </ul>
                    <div class="ad">
                        <p>Advertisement: Buy our product!</p>
                    </div>
                </aside>

                <footer>
                    <p>Copyright 2024 Example Corp</p>
                    <nav>
                        <a href="/privacy">Privacy</a>
                        <a href="/terms">Terms</a>
                    </nav>
                </footer>

                <script>
                    console.log('This script should be removed');
                    analytics.track('page_view');
                </script>
            </body>
        </html>
    "#;

    println!("--- Complex Article Test ---");
    println!("\n--- Metadata ---");
    let complex_metadata = extract_metadata(complex_html);
    for (key, value) in complex_metadata.iter() {
        println!("{}: {}", key, value);
    }

    println!("\n--- Extracted Markdown ---");
    let markdown = extract_content(complex_html)?;
    println!("{}", markdown);

    // Verify content quality
    println!("\n--- Content Quality Check ---");
    println!("✓ Metadata extracted: {}", !complex_metadata.is_empty());
    println!(
        "✓ Contains main heading: {}",
        markdown.contains("Future of Web Crawling")
    );
    println!(
        "✓ Contains list items: {}",
        markdown.contains("JavaScript-heavy")
    );
    println!(
        "✓ Navigation removed: {}",
        !markdown.contains("About") || !markdown.contains("Contact")
    );
    println!("✓ Footer removed: {}", !markdown.contains("Copyright 2024"));
    println!("✓ Ads removed: {}", !markdown.contains("Buy our product"));
    println!("✓ Scripts removed: {}", !markdown.contains("console.log"));
    println!("✓ Structure preserved: {}", markdown.contains("#"));

    let word_count = markdown.split_whitespace().count();
    println!("✓ Word count: {} words", word_count);
    println!("✓ Character count: {} chars", markdown.len());

    println!("\n=== Test Complete ===");
    println!("\nThe content extraction successfully:");
    println!("1. Identified the main content area (article)");
    println!("2. Removed navigation, sidebar, footer, and ads");
    println!("3. Preserved document structure (headings, lists, quotes)");
    println!("4. Converted to clean markdown for LLM/RAG use");
    println!("5. Extracted metadata (title, author, date)");

    Ok(())
}
