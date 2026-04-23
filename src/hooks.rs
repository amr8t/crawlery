//! Lifecycle hook execution for the crawler.

use anyhow::Result;
use std::collections::HashMap;

use crate::{Hook, HookType};

/// Run a list of hooks, passing env_vars as environment variables.
/// Non-fatal unless abort_on_error is set.
pub async fn run_hooks(hooks: &[Hook], env_vars: &HashMap<String, String>) -> Result<()> {
    for hook in hooks {
        if let Err(e) = run_hook(hook, env_vars).await {
            if hook.abort_on_error {
                return Err(e);
            }
            eprintln!("Warning: Hook failed (continuing): {}", e);
        }
    }
    Ok(())
}

async fn run_hook(hook: &Hook, env_vars: &HashMap<String, String>) -> Result<()> {
    match &hook.hook_type {
        HookType::Command { cmd, args } => {
            let timeout_ms = hook.timeout_ms.unwrap_or(5000);
            run_command_hook(cmd, args, env_vars, timeout_ms).await
        }
        HookType::Javascript { .. } => {
            // JS hooks are executed in browser context by browser.rs -- skip here
            Ok(())
        }
    }
}

async fn run_command_hook(
    cmd: &str,
    args: &[String],
    env_vars: &HashMap<String, String>,
    timeout_ms: u64,
) -> Result<()> {
    let mut command = tokio::process::Command::new(cmd);
    command.args(args);
    for (k, v) in env_vars {
        command.env(k, v);
    }
    let duration = tokio::time::Duration::from_millis(timeout_ms);
    match tokio::time::timeout(duration, command.status()).await {
        Ok(Ok(status)) => {
            if !status.success() {
                anyhow::bail!("Hook command '{}' exited with status: {}", cmd, status);
            }
            Ok(())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!("Hook command '{}' failed to spawn: {}", cmd, e)),
        Err(_) => anyhow::bail!("Hook command '{}' timed out after {}ms", cmd, timeout_ms),
    }
}
