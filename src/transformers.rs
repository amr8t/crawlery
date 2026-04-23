//! Result transformers applied after crawling, before output.

use anyhow::Result;
use evalexpr::ContextWithMutableVariables;
use std::collections::HashSet;

use crate::{CrawlResult, FilterCondition, Transformer};

/// Apply a sequence of transformers to crawl results.
pub async fn apply_transformers(
    results: Vec<CrawlResult>,
    transformers: &[Transformer],
) -> Result<Vec<CrawlResult>> {
    let mut current = results;
    for transformer in transformers {
        current = apply_one(current, transformer).await?;
    }
    Ok(current)
}

async fn apply_one(results: Vec<CrawlResult>, t: &Transformer) -> Result<Vec<CrawlResult>> {
    match t {
        Transformer::Filter { condition } => Ok(filter_results(results, condition)),
        Transformer::Deduplicator { field } => Ok(dedup_results(results, field)),
        Transformer::ExtractFields { .. } => Ok(results), // Applied at output level
        Transformer::Command { cmd, args, timeout_ms } => {
            run_command_transformer(results, cmd, args, *timeout_ms).await
        }
    }
}

fn filter_results(results: Vec<CrawlResult>, condition: &FilterCondition) -> Vec<CrawlResult> {
    results.into_iter().filter(|r| eval_condition(r, condition)).collect()
}

fn eval_condition(result: &CrawlResult, condition: &FilterCondition) -> bool {
    let expr = condition
        .expression
        .replace("content.len()", &result.content.len().to_string())
        .replace("links.len()", &result.links.len().to_string())
        .replace("errors.len()", &result.errors.len().to_string())
        .replace("url.len()", &result.url.len().to_string());

    let mut ctx = evalexpr::HashMapContext::new();
    let _ = ctx.set_value(
        "status_code".to_string(),
        evalexpr::Value::Int(result.status_code.unwrap_or(0) as i64),
    );
    let _ = ctx.set_value("depth".to_string(), evalexpr::Value::Int(result.depth as i64));
    let _ = ctx.set_value(
        "content_len".to_string(),
        evalexpr::Value::Int(result.content.len() as i64),
    );

    match evalexpr::eval_boolean_with_context(&expr, &ctx) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Warning: Filter expression evaluation failed (keeping result): {}", e);
            true
        }
    }
}

fn dedup_results(results: Vec<CrawlResult>, field: &str) -> Vec<CrawlResult> {
    let mut seen: HashSet<String> = HashSet::new();
    results
        .into_iter()
        .filter(|r| {
            let key = match field {
                "url" => r.url.clone(),
                "title" => r.title.clone().unwrap_or_default(),
                "content" => r.content.clone(),
                _ => r.metadata.get(field).cloned().unwrap_or_else(|| r.url.clone()),
            };
            seen.insert(key)
        })
        .collect()
}

async fn run_command_transformer(
    results: Vec<CrawlResult>,
    cmd: &str,
    args: &[String],
    timeout_ms: Option<u64>,
) -> Result<Vec<CrawlResult>> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    let input_json = serde_json::to_string(&results)?;
    let timeout_dur = tokio::time::Duration::from_millis(timeout_ms.unwrap_or(10_000));

    let mut child = tokio::process::Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn command transformer '{}': {}", cmd, e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_json.as_bytes()).await?;
    }

    let output = tokio::time::timeout(timeout_dur, child.wait_with_output())
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Command transformer '{}' timed out after {}ms",
                cmd,
                timeout_ms.unwrap_or(10_000)
            )
        })??;

    if !output.stderr.is_empty() {
        eprintln!(
            "Command transformer stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let transformed: Vec<CrawlResult> =
        serde_json::from_slice(&output.stdout).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse command transformer output as CrawlResult array: {}",
                e
            )
        })?;

    Ok(transformed)
}

/// Project crawl results to only the named fields (for extract_fields / output).
/// Returns serde_json::Value objects with only the requested top-level fields.
pub fn project_fields(results: &[CrawlResult], fields: &[String]) -> Vec<serde_json::Value> {
    if fields.is_empty() {
        return results
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
            .collect();
    }
    results
        .iter()
        .map(|r| {
            let full = serde_json::to_value(r)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            let mut map = serde_json::Map::new();
            for field in fields {
                // Support simple dot-notation for metadata sub-fields
                let parts: Vec<&str> = field.splitn(2, '.').collect();
                if parts.len() == 2 {
                    if let Some(obj) = full.get(parts[0]) {
                        if let Some(val) = obj.get(parts[1]) {
                            map.insert(field.clone(), val.clone());
                            continue;
                        }
                    }
                }
                if let Some(val) = full.get(field.as_str()) {
                    map.insert(field.clone(), val.clone());
                }
            }
            serde_json::Value::Object(map)
        })
        .collect()
}
