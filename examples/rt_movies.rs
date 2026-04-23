//! Rotten Tomatoes Movie Synopsis Crawler
//!
//! Demonstrates Crawlery's programmatic Pipeline API for multi-stage crawling:
//!
//!   Stage 1: discover.yaml  — Browser mode + JS scroll hook + Python link extractor
//!   Stage 2: extract.yaml   — HTTP mode + readability + Python synopsis cleaner
//!
//!   Rust Pipeline orchestrates:
//!     - Conditional execution (.when guard)
//!     - Fork-join result partitioning
//!     - Multi-file output
//!
//! Run:    cargo run --example rt_movies
//! Output: out/rt/rt_rich.json   — movies with synopsis
//!         out/rt/rt_sparse.json — pages where extraction was sparse

use anyhow::Result;
use crawlery::{pipeline::Pipeline, CrawlResult};
use std::{fs, path::Path};

#[tokio::main]
async fn main() -> Result<()> {
    fs::create_dir_all("out/rt")?;

    println!("═══ Rotten Tomatoes Movie Crawler ══════════════════════════════════");

    // ── Programmatic Pipeline ─────────────────────────────────────────────────
    // YAML defines individual stage configs (what to crawl, how to transform).
    // Rust code defines pipeline orchestration (when to skip, how to partition).

    let results = Pipeline::new()
        .stage("discover", "examples/rt/discover.yaml")
        // discover.yaml:
        //   - Browser mode with JS scroll hook
        //   - Python transformer reshapes links → movie URLs
        .end()
        .stage("extract", "examples/rt/extract.yaml")
        // extract.yaml:
        //   - HTTP mode with md_readability
        //   - Python transformer isolates synopsis
        .when(|prev| {
            // Conditional execution: skip if nothing discovered
            if prev.is_empty() {
                eprintln!("  [extract] SKIPPED — no movie URLs discovered");
                return false;
            }
            println!("  Queued {} movie URLs for extraction", prev.len());
            true
        })
        .end()
        .run()
        .await?;

    println!("\n  Extracted {} movie page(s)", results.len());

    // ── Fork-Join ─────────────────────────────────────────────────────────────
    // Python transformer sets metadata["has_synopsis"] = "true"|"false".
    // Partition results by quality for separate downstream use.

    let (rich, sparse): (Vec<CrawlResult>, Vec<CrawlResult>) = results.into_iter().partition(|r| {
        r.metadata
            .get("has_synopsis")
            .map(|v| v == "true")
            .unwrap_or(false)
    });

    println!(
        "  Fork-join → {} with synopsis / {} sparse",
        rich.len(),
        sparse.len()
    );

    // ── Output ────────────────────────────────────────────────────────────────
    write_output("out/rt/rt_rich.json", &rich)?;
    write_output("out/rt/rt_sparse.json", &sparse)?;

    // Human-readable preview
    if let Some(first) = rich.first() {
        let title = first.title.as_deref().unwrap_or("(no title)");
        let synopsis = first
            .metadata
            .get("synopsis")
            .map(|s| s.as_str())
            .unwrap_or("(none)");
        println!("\n── Preview: {title} ──");
        println!("URL:      {}", first.url);
        println!("Synopsis: {}", &synopsis[..synopsis.len().min(300)]);
    }

    println!("\n══ Done ══════════════════════════════════════════════════════════════");
    println!(
        "  out/rt/rt_rich.json   — {} movie(s) with synopsis",
        rich.len()
    );
    println!("  out/rt/rt_sparse.json — {} sparse page(s)", sparse.len());

    Ok(())
}

/// Serialize results to an indented JSON file.
fn write_output(path: &str, results: &[CrawlResult]) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(results)?)?;
    println!("  -> wrote {} record(s) to {path}", results.len());
    Ok(())
}
