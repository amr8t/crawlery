//! Before/After comparison of content extraction methods.
//!
//! This example demonstrates the improvement from simple HTML tag stripping
//! to readability-style content extraction optimized for RAG/LLM use cases.

use crawlery::content::{extract_content, extract_metadata};

/// Old-style content extraction: simple tag stripping
fn old_clean_content(html: &str) -> String {
    let mut cleaned = html.to_string();

    // Remove scripts and styles
    let patterns = [
        r"(?s)<script[^>]*>.*?</script>",
        r"(?s)<style[^>]*>.*?</style>",
    ];

    for pattern in &patterns {
        let re = regex::Regex::new(pattern).unwrap();
        cleaned = re.replace_all(&cleaned, "").to_string();
    }

    // Strip all HTML tags
    let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
    cleaned = tag_re.replace_all(&cleaned, " ").to_string();

    // Clean whitespace
    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn main() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  CONTENT EXTRACTION: BEFORE vs AFTER COMPARISON                 ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Example 1: Simple blog post
    let blog_html = r#"
        <html>
            <head>
                <title>10 Rust Tips for Beginners</title>
                <meta name="author" content="John Developer">
                <meta name="description" content="Essential tips for learning Rust">
            </head>
            <body>
                <header>
                    <nav>
                        <a href="/">Home</a>
                        <a href="/blog">Blog</a>
                        <a href="/about">About</a>
                        <a href="/contact">Contact</a>
                    </nav>
                </header>

                <main>
                    <article>
                        <h1>10 Rust Tips for Beginners</h1>
                        <p class="byline">By John Developer</p>

                        <p>Learning Rust can be challenging, but these tips will help you
                        get started on the right foot.</p>

                        <h2>1. Embrace the Borrow Checker</h2>
                        <p>Don't fight the borrow checker - it's your friend! It prevents
                        data races and memory issues at compile time.</p>

                        <h2>2. Use Clippy</h2>
                        <p>Clippy is an amazing linting tool that teaches you Rust idioms
                        and best practices.</p>

                        <h2>3. Read "The Book"</h2>
                        <p>The official Rust book is excellent and freely available online.</p>
                    </article>
                </main>

                <aside class="sidebar">
                    <h3>Popular Posts</h3>
                    <ul>
                        <li><a href="/post1">Why Rust?</a></li>
                        <li><a href="/post2">Getting Started</a></li>
                        <li><a href="/post3">Advanced Patterns</a></li>
                    </ul>
                    <div class="advertisement">
                        <h4>Learn Rust Online!</h4>
                        <p>Join our premium course for only $99!</p>
                        <button>Sign Up Now</button>
                    </div>
                </aside>

                <footer>
                    <p>Copyright 2024 RustBlog. All rights reserved.</p>
                    <nav>
                        <a href="/privacy">Privacy Policy</a>
                        <a href="/terms">Terms of Service</a>
                    </nav>
                </footer>

                <script>
                    analytics.track('page_view', { page: 'blog_post' });
                    console.log('User visited blog post');
                </script>
            </body>
        </html>
    "#;

    println!("═══════════════════════════════════════════════════════════════════");
    println!("Example 1: Blog Post");
    println!("═══════════════════════════════════════════════════════════════════\n");

    println!("─── OLD METHOD (Simple Tag Stripping) ───\n");
    let old_result = old_clean_content(blog_html);
    println!("{}\n", old_result);
    println!("Word count: {}", old_result.split_whitespace().count());
    println!(
        "Contains navigation: {}",
        old_result.contains("Home") && old_result.contains("Contact")
    );
    println!("Contains footer: {}", old_result.contains("Copyright 2024"));
    println!("Contains ads: {}", old_result.contains("$99"));
    println!("Contains sidebar: {}", old_result.contains("Popular Posts"));

    println!("\n─── NEW METHOD (Readability Extraction) ───\n");
    let new_result = extract_content(blog_html)?;
    println!("{}\n", new_result);
    println!("Word count: {}", new_result.split_whitespace().count());
    println!(
        "Contains navigation: {}",
        new_result.contains("Home") && new_result.contains("Contact")
    );
    println!("Contains footer: {}", new_result.contains("Copyright 2024"));
    println!("Contains ads: {}", new_result.contains("$99"));
    println!("Contains sidebar: {}", new_result.contains("Popular Posts"));

    println!("\n─── METADATA EXTRACTION ───\n");
    let metadata = extract_metadata(blog_html);
    for (key, value) in metadata.iter() {
        println!("{}: {}", key, value);
    }

    // Example 2: News article with complex layout
    let news_html = r#"
        <html>
            <head>
                <title>Breaking: Major Technology Breakthrough</title>
                <meta property="og:title" content="Major Technology Breakthrough Announced">
                <meta property="og:description" content="Scientists achieve quantum computing milestone">
                <meta name="author" content="Sarah Journalist">
                <meta property="article:published_time" content="2024-01-20T10:30:00Z">
            </head>
            <body>
                <div class="cookie-banner">
                    This site uses cookies. <button>Accept</button>
                </div>

                <header class="site-header">
                    <div class="logo">TechNews Daily</div>
                    <nav class="main-nav">
                        <a href="/">Home</a>
                        <a href="/tech">Technology</a>
                        <a href="/science">Science</a>
                        <a href="/business">Business</a>
                    </nav>
                </header>

                <div class="content-wrapper">
                    <article class="main-content">
                        <h1>Major Technology Breakthrough Announced</h1>

                        <div class="article-meta">
                            <span class="author">Sarah Journalist</span>
                            <time datetime="2024-01-20">January 20, 2024</time>
                        </div>

                        <p class="lead">Scientists have achieved a major breakthrough in quantum
                        computing that could revolutionize the technology industry within the
                        next decade.</p>

                        <p>The research team at QuantumLab announced today that they have
                        successfully demonstrated a quantum processor with 1000 stable qubits,
                        far exceeding previous records.</p>

                        <h2>What This Means</h2>

                        <p>This breakthrough has several important implications:</p>

                        <ul>
                            <li>Faster drug discovery and medical research</li>
                            <li>Enhanced cryptography and security</li>
                            <li>Advanced AI and machine learning capabilities</li>
                            <li>Complex system simulation and optimization</li>
                        </ul>

                        <blockquote>
                            "This is a game-changer for the entire field," said Dr. Jane Smith,
                            lead researcher on the project.
                        </blockquote>

                        <h2>Technical Details</h2>

                        <p>The team used a novel error-correction technique that maintains
                        quantum coherence for unprecedented durations. This allows for more
                        complex calculations without quantum decoherence degrading the results.</p>
                    </article>

                    <aside class="related-content">
                        <h3>Related Stories</h3>
                        <div class="story-card">
                            <a href="/story1">Quantum Computing Basics</a>
                        </div>
                        <div class="story-card">
                            <a href="/story2">The Future of AI</a>
                        </div>
                    </aside>
                </div>

                <div class="ad-banner">
                    <img src="ad.jpg" alt="Advertisement">
                    <p>Special Offer: Subscribe Now!</p>
                </div>

                <footer class="site-footer">
                    <div class="footer-nav">
                        <a href="/about">About Us</a>
                        <a href="/contact">Contact</a>
                        <a href="/advertise">Advertise</a>
                    </div>
                    <p>&copy; 2024 TechNews Daily. All rights reserved.</p>
                </footer>
            </body>
        </html>
    "#;

    println!("\n\n═══════════════════════════════════════════════════════════════════");
    println!("Example 2: News Article with Complex Layout");
    println!("═══════════════════════════════════════════════════════════════════\n");

    println!("─── OLD METHOD (Simple Tag Stripping) ───\n");
    let old_news = old_clean_content(news_html);
    println!("{}\n", old_news);
    println!("Word count: {}", old_news.split_whitespace().count());
    println!(
        "Contains cookie banner: {}",
        old_news.contains("This site uses cookies")
    );
    println!(
        "Contains navigation: {}",
        old_news.contains("Technology") && old_news.contains("Business")
    );

    println!("\n─── NEW METHOD (Readability Extraction) ───\n");
    let new_news = extract_content(news_html)?;
    println!("{}\n", new_news);
    println!("Word count: {}", new_news.split_whitespace().count());
    println!(
        "Contains cookie banner: {}",
        new_news.contains("This site uses cookies")
    );
    println!(
        "Contains navigation: {}",
        new_news.contains("Technology") && new_news.contains("Business")
    );

    println!("\n─── METADATA EXTRACTION ───\n");
    let news_metadata = extract_metadata(news_html);
    for (key, value) in news_metadata.iter() {
        println!("{}: {}", key, value);
    }

    // Summary comparison
    println!("\n\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║  SUMMARY: KEY IMPROVEMENTS                                      ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    println!("✓ Boilerplate Removal:");
    println!("  • Navigation menus automatically filtered out");
    println!("  • Footer content removed");
    println!("  • Sidebar widgets excluded");
    println!("  • Advertisement blocks stripped");
    println!("  • Cookie banners eliminated\n");

    println!("✓ Content Quality:");
    println!("  • Main article content identified and extracted");
    println!("  • Document structure preserved (headings, lists)");
    println!("  • Clean markdown format for LLM ingestion");
    println!("  • Reduced noise improves RAG accuracy\n");

    println!("✓ Metadata Extraction:");
    println!("  • Title extraction from multiple sources");
    println!("  • Author information captured");
    println!("  • Publication dates preserved");
    println!("  • OpenGraph tags prioritized\n");

    println!("✓ Use Cases:");
    println!("  • Better vector embeddings for RAG systems");
    println!("  • Cleaner input for LLM summarization");
    println!("  • Improved semantic search results");
    println!("  • More accurate content analysis\n");

    println!("─────────────────────────────────────────────────────────────────\n");
    println!("The new readability-style extraction provides significantly");
    println!("cleaner output that is ideal for RAG/LLM applications compared");
    println!("to simple HTML tag stripping.\n");

    Ok(())
}
