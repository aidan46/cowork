use std::path::{Path, PathBuf};

use crate::error::AppError;

pub fn validate_ask_paths(paths: &[PathBuf], recursive: bool) -> Result<(), AppError> {
    for path in paths {
        validate_path(path, recursive)?;
    }

    Ok(())
}

fn validate_path(path: &Path, recursive: bool) -> Result<(), AppError> {
    if !path.exists() {
        return Err(AppError::missing_path(path));
    }

    if path.is_dir() && !recursive {
        return Err(AppError::directory_requires_recursive(path));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::PathBuf,
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::error::AppError;

    use super::validate_ask_paths;

    #[test]
    fn missing_path_returns_missing_path_error() {
        let path = unique_path("missing");

        let error =
            validate_ask_paths(std::slice::from_ref(&path), false).expect_err("path should fail");

        match error {
            AppError::MissingPath { path: error_path } => {
                assert_eq!(error_path, path.display().to_string());
            }
            other => panic!("expected missing path error, got {other:?}"),
        }
    }

    #[test]
    fn dir_requires_recursive_flag() {
        let dir = unique_path("dir");
        fs::create_dir(&dir).expect("dir should create");

        let error =
            validate_ask_paths(std::slice::from_ref(&dir), false).expect_err("dir should fail");

        match error {
            AppError::DirectoryRequiresRecursive { path } => {
                assert_eq!(path, dir.display().to_string());
            }
            other => panic!("expected recursive error, got {other:?}"),
        }

        fs::remove_dir_all(dir).expect("dir should clean");
    }

    #[test]
    fn file_path_passes_gate() {
        let dir = unique_path("file-dir");
        let file = dir.join("input.txt");
        fs::create_dir(&dir).expect("dir should create");
        fs::write(&file, "stub").expect("file should write");

        validate_ask_paths(&[file], false).expect("file path should pass");

        fs::remove_dir_all(dir).expect("dir should clean");
    }

    fn unique_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();

        env::temp_dir().join(format!("cowork-{label}-{}-{nanos}", process::id()))
    }
}
