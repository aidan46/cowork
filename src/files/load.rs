use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::error::AppError;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LoadedAskFile {
    pub(crate) path: PathBuf,
    pub(crate) content: String,
    pub(crate) bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LoadedAskFiles {
    pub(crate) files: Vec<LoadedAskFile>,
    pub(crate) total_bytes: usize,
}

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

fn is_binary_file(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}
