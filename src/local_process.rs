//! Bounded, cancellable local subprocesses shared by media adapters.
use anyhow::{Context, Result, bail};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
pub async fn run(mut command: Command, seconds: u64, step: &str) -> Result<()> {
    // Dropping an in-flight SSE stream cancels inference and removes temporary files.
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = tokio::time::timeout(Duration::from_secs(seconds), command.status())
        .await
        .with_context(|| format!("{step} timed out"))?
        .with_context(|| format!("Cannot start {step}"))?;
    if !status.success() {
        bail!("{step} failed; check the file and local model installation");
    }
    Ok(())
}

pub fn executable_exists(binary: &str) -> bool {
    fn executable(path: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            path.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            path.is_file()
        }
    }
    if Path::new(binary).components().count() > 1 {
        return executable(Path::new(binary));
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|p| executable(&p.join(binary))))
}

/// Read at most `limit` stdout bytes; never accumulate arbitrary tool output.
pub async fn output(
    mut command: Command,
    seconds: u64,
    limit: usize,
    step: &str,
) -> Result<Vec<u8>> {
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let task = async {
        let mut child = command
            .spawn()
            .with_context(|| format!("Cannot start {step}"))?;
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .context("Missing output pipe")?
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > limit {
            bail!("{step} output exceeds the limit");
        }
        if !child.wait().await?.success() {
            bail!("{step} failed. The file may be corrupt, encrypted or unsupported.");
        }
        Ok(bytes)
    };
    tokio::time::timeout(Duration::from_secs(seconds), task)
        .await
        .with_context(|| format!("{step} timed out"))?
}
