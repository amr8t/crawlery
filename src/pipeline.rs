//! Pipeline builder for composing multi-stage crawl workflows in code.
//!
//! Provides a fluent Rust API for chaining crawl stages with conditional execution,
//! per-stage result transforms, and automatic URL forwarding between stages.
//!
//! # Example
//! ```no_run
//! use crawlery::pipeline::Pipeline;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let results = Pipeline::new()
//!         .stage("discover", "recipes/discover.yaml").end()
//!         .stage("extract", "recipes/extract.yaml")
//!             .when(|prev| !prev.is_empty())
//!         .end()
//!         .run().await?;
//!     println!("Got {} results", results.len());
//!     Ok(())
//! }
//! ```

use crate::{CrawlConfig, CrawlResult, Crawler};
use anyhow::Result;
use std::path::PathBuf;

type WhenFn = Box<dyn Fn(&[CrawlResult]) -> bool + Send + Sync>;
type TransformFn = Box<dyn Fn(Vec<CrawlResult>) -> Vec<CrawlResult> + Send + Sync>;

struct Stage {
    name: String,
    recipe: PathBuf,
    when: Option<WhenFn>,
    transform: Option<TransformFn>,
}

/// Multi-stage crawl pipeline with a fluent builder API.
///
/// Build a pipeline with [`Pipeline::new`], add stages via [`.stage()`][Pipeline::stage],
/// and execute with [`.run()`][Pipeline::run].
///
/// Each stage's results are automatically forwarded to the next stage as URL inputs
/// (written to a temp file and injected via `input_from`). Use `.when()` to skip a
/// stage conditionally and `.transform()` to filter or reshape results in-process.
pub struct Pipeline {
    stages: Vec<Stage>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Add a named stage that loads its configuration from a YAML recipe file.
    ///
    /// Returns a [`StageBuilder`] — call [`.end()`][StageBuilder::end] to commit
    /// the stage and return to the pipeline for further chaining.
    pub fn stage(self, name: &str, recipe: impl Into<PathBuf>) -> StageBuilder {
        StageBuilder {
            pipeline: self,
            stage: Stage {
                name: name.to_string(),
                recipe: recipe.into(),
                when: None,
                transform: None,
            },
        }
    }

    /// Execute all stages sequentially, forwarding each stage's results as URL inputs
    /// to the next stage.
    ///
    /// - If a stage's `.when()` predicate returns `false`, that stage is skipped and
    ///   the previous results pass unchanged to the next stage.
    /// - A `.transform()` closure, if set, is applied to a stage's results before
    ///   they are forwarded.
    /// - Temp files written for URL forwarding are cleaned up after the run completes.
    pub async fn run(self) -> Result<Vec<CrawlResult>> {
        let mut prev: Vec<CrawlResult> = Vec::new();
        let mut temp_files: Vec<PathBuf> = Vec::new();

        for stage in self.stages {
            // Evaluate guard condition before touching the recipe file
            if let Some(ref when) = stage.when {
                if !when(&prev) {
                    eprintln!("Pipeline: stage '{}' skipped", stage.name);
                    continue;
                }
            }

            let mut config = CrawlConfig::from_file(&stage.recipe)?;

            // Forward previous results as URL inputs when not overridden by the recipe
            if !prev.is_empty() && config.input_from.is_none() {
                let temp_path = std::env::temp_dir()
                    .join(format!("crawlery_{}.json", sanitize_name(&stage.name)));
                let urls: Vec<serde_json::Value> = prev
                    .iter()
                    .map(|r| serde_json::json!({ "url": r.url, "title": r.title }))
                    .collect();
                std::fs::write(&temp_path, serde_json::to_string(&urls)?)?;
                config.url = String::new();
                config.input_from = Some(temp_path.clone());
                temp_files.push(temp_path);
            }

            eprintln!("Pipeline: running stage '{}'", stage.name);
            let mut results = Crawler::new(config).crawl().await?;

            if let Some(transform) = stage.transform {
                results = transform(results);
            }
            prev = results;
        }

        for f in temp_files {
            let _ = std::fs::remove_file(f);
        }

        Ok(prev)
    }
}

/// Builder for a single pipeline stage.
///
/// Obtained from [`Pipeline::stage`]. Configure the stage with `.when()` and
/// `.transform()`, then call [`.end()`][StageBuilder::end] to commit it.
pub struct StageBuilder {
    pipeline: Pipeline,
    stage: Stage,
}

impl StageBuilder {
    /// Only run this stage when `f` returns `true` for the previous stage's results.
    ///
    /// If the predicate returns `false`, the stage is skipped and the previous results
    /// are forwarded unchanged to the next stage.
    pub fn when<F: Fn(&[CrawlResult]) -> bool + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.stage.when = Some(Box::new(f));
        self
    }

    /// Apply `f` to this stage's results before forwarding them to the next stage.
    ///
    /// Useful for filtering, deduplication, or reshaping results in-process without
    /// needing a separate recipe transformer.
    pub fn transform<F: Fn(Vec<CrawlResult>) -> Vec<CrawlResult> + Send + Sync + 'static>(
        mut self,
        f: F,
    ) -> Self {
        self.stage.transform = Some(Box::new(f));
        self
    }

    /// Commit this stage to the pipeline and return the pipeline for further chaining.
    pub fn end(mut self) -> Pipeline {
        self.pipeline.stages.push(self.stage);
        self.pipeline
    }
}

fn sanitize_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_pipeline_returns_empty() {
        let results = Pipeline::new().run().await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_when_false_skips_stage_without_loading_recipe() {
        // The recipe path is intentionally invalid — since `when` is false
        // we never try to load or crawl it, so no error occurs.
        let results = Pipeline::new()
            .stage("skipped", "nonexistent_recipe.yaml")
            .when(|_| false)
            .end()
            .run()
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_transform_not_called_when_stage_skipped() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let results = Pipeline::new()
            .stage("skipped", "nonexistent.yaml")
            .when(|_| false)
            .transform(move |r| {
                called_clone.store(true, Ordering::SeqCst);
                r
            })
            .end()
            .run()
            .await
            .unwrap();

        assert!(results.is_empty());
        assert!(
            !called.load(Ordering::SeqCst),
            "transform must not be called on a skipped stage"
        );
    }

    #[tokio::test]
    async fn test_multiple_skipped_stages_return_empty() {
        let results = Pipeline::new()
            .stage("stage1", "nonexistent1.yaml")
            .when(|_| false)
            .end()
            .stage("stage2", "nonexistent2.yaml")
            .when(|_| false)
            .end()
            .run()
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    /// Full two-stage pipeline — requires network and recipe files.
    /// Run with: `cargo test -- --include-ignored`
    #[tokio::test]
    #[ignore]
    async fn test_two_stage_pipeline_integration() {
        let results = Pipeline::new()
            .stage("discover", "examples/recipes/discover.yaml")
            .end()
            .stage("extract", "examples/recipes/extract.yaml")
            .when(|prev| !prev.is_empty())
            .end()
            .run()
            .await
            .unwrap();
        println!("Integration: {} results", results.len());
    }

    /// Transform modifies results — requires a real stage to produce results.
    /// Run with: `cargo test -- --include-ignored`
    #[tokio::test]
    #[ignore]
    async fn test_transform_modifies_results() {
        let results = Pipeline::new()
            .stage("crawl", "examples/recipes/discover.yaml")
            .transform(|mut r| {
                r.truncate(1);
                r
            })
            .end()
            .run()
            .await
            .unwrap();
        assert!(results.len() <= 1);
    }
}
