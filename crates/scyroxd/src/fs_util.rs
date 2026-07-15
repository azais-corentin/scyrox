//! Filesystem helpers shared across daemon modules.

use std::path::Path;

use anyhow::Result;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Atomically write `contents` to `path`.
///
/// Writes to a sibling `<path>.tmp` file, flushes it to disk with `sync_all`,
/// then renames it over `path`. A crash mid-write leaves either the old file or
/// the fully-written new file, never a truncated one. On any failure after the
/// temp file is created, the temp file is removed on a best-effort basis.
pub(crate) async fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let tmp = path.with_extension("tmp");

    if let Err(e) = write_tmp(&tmp, contents).await {
        let _ = fs::remove_file(&tmp).await;
        return Err(e);
    }

    if let Err(e) = fs::rename(&tmp, path).await {
        let _ = fs::remove_file(&tmp).await;
        return Err(e.into());
    }

    Ok(())
}

/// Write and durably flush the temp file.
async fn write_tmp(tmp: &Path, contents: &str) -> Result<()> {
    let mut file = fs::File::create(tmp).await?;
    file.write_all(contents.as_bytes()).await?;
    file.sync_all().await?;
    Ok(())
}
