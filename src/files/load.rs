use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::AppError;

#[derive(Debug, PartialEq, Eq)]
/// One loaded ask file.
pub(crate) struct LoadedAskFile {
    /// Source path.
    pub(crate) path: PathBuf,
    /// UTF-8 file content.
    pub(crate) content: String,
    /// Content byte count.
    pub(crate) bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
/// Loaded ask file set.
pub(crate) struct LoadedAskFiles {
    /// Loaded files in input order.
    pub(crate) files: Vec<LoadedAskFile>,
    /// Total byte count.
    pub(crate) total_bytes: usize,
}

/// Load ask files and total bytes.
///
/// # Errors
///
/// Returns [`AppError`] on file read failure or when total bytes exceed the cap.
pub(crate) fn load_ask_files(
    paths: &[PathBuf],
    max_bytes: Option<usize>,
) -> Result<LoadedAskFiles, AppError> {
    let mut files = Vec::new();
    let mut total_bytes = 0;

    for path in paths {
        let Some(file) = load_ask_file(path)? else {
            continue;
        };

        total_bytes += file.bytes;
        if let Some(max_bytes) = max_bytes
            && total_bytes > max_bytes
        {
            return Err(AppError::max_bytes_exceeded(max_bytes, total_bytes));
        }

        files.push(file);
    }

    Ok(LoadedAskFiles { files, total_bytes })
}

/// Load one ask file when text.
///
/// # Errors
///
/// Returns [`AppError`] when the file cannot be read.
fn load_ask_file(path: &Path) -> Result<Option<LoadedAskFile>, AppError> {
    if path.is_symlink() || !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(path).map_err(|error| AppError::file_read(path, &error))?;
    if is_binary_file(&bytes) {
        return Ok(None);
    }

    let content = match String::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };

    let bytes = content.len();
    Ok(Some(LoadedAskFile {
        path: path.to_path_buf(),
        content,
        bytes,
    }))
}

/// Detect binary by NUL byte.
fn is_binary_file(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}
