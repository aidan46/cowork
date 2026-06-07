/// Candidate discovery.
mod discovery;
/// File load helpers.
mod load;

pub use discovery::collect_ask_candidates;
pub(crate) use load::{LoadedAskFile, LoadedAskFiles, load_ask_files};

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::error::AppError;

    use super::{collect_ask_candidates, discovery::validate_ask_paths, load_ask_files};

    #[test]
    fn missing_path_returns_missing_path_error() {
        let path = unique_path("missing");

        let error = validate_ask_paths(std::slice::from_ref(&path), false, true)
            .expect_err("path should fail");

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

        let error = validate_ask_paths(std::slice::from_ref(&dir), false, true)
            .expect_err("dir should fail");

        match error {
            AppError::DirectoryRequiresRecursive { path } => {
                assert_eq!(path, dir.display().to_string());
            }
            other => panic!("expected recursive error, got {other:?}"),
        }

        fs::remove_dir_all(dir).expect("dir should clean");
    }

    #[test]
    fn recursive_dir_walk_collects_files() {
        let dir = TestDir::new("recursive");
        let alpha = dir.path.join("src/alpha.rs");
        let beta = dir.path.join("src/nested/beta.txt");
        write_file(&alpha);
        write_file(&beta);

        let candidates =
            collect_ask_candidates(std::slice::from_ref(&dir.path), true, &[], &[], true)
                .expect("recursive walk should pass");

        assert_eq!(candidates, vec![alpha, beta]);
    }

    #[test]
    fn default_dir_pruning_skips_excluded_dirs() {
        let dir = TestDir::new("prune");
        let keep = dir.path.join("src/keep.rs");
        let git_file = dir.path.join(".git/config");
        let module_file = dir.path.join("node_modules/pkg/index.js");
        write_file(&keep);
        write_file(&git_file);
        write_file(&module_file);

        let candidates =
            collect_ask_candidates(std::slice::from_ref(&dir.path), true, &[], &[], true)
                .expect("recursive walk should pass");

        assert_eq!(candidates, vec![keep]);
    }

    #[test]
    fn include_and_exclude_matching_applies_to_discovered_files() {
        let dir = TestDir::new("globs");
        let keep = dir.path.join("src/lib.rs");
        let skip_by_exclude = dir.path.join("src/lib_test.rs");
        let skip_by_include = dir.path.join("src/readme.md");
        write_file(&keep);
        write_file(&skip_by_exclude);
        write_file(&skip_by_include);

        let candidates = collect_ask_candidates(
            std::slice::from_ref(&dir.path),
            true,
            &["*.rs".to_string()],
            &["*test.rs".to_string()],
            true,
        )
        .expect("glob filtering should pass");

        assert_eq!(candidates, vec![keep]);
    }

    #[test]
    fn explicit_file_bypasses_include() {
        let dir = TestDir::new("explicit-file");
        let file = dir.path.join("Cargo.toml");
        write_file(&file);

        let candidates = collect_ask_candidates(
            std::slice::from_ref(&file),
            false,
            &["*.rs".to_string()],
            &[],
            true,
        )
        .expect("explicit file should pass");

        assert_eq!(candidates, vec![file]);
    }

    #[test]
    fn file_path_passes_gate() {
        let dir = TestDir::new("file-dir");
        let file = dir.path.join("input.txt");
        write_file(&file);

        validate_ask_paths(&[file], false, true).expect("file path should pass");
    }

    #[test]
    fn strict_mixed_paths_return_missing_path_error() {
        let dir = TestDir::new("strict-mixed");
        let existing = dir.path.join("input.txt");
        let missing = dir.path.join("missing.txt");
        write_file(&existing);

        let error = collect_ask_candidates(&[existing, missing.clone()], false, &[], &[], true)
            .expect_err("missing path should fail");

        match error {
            AppError::MissingPath { path } => {
                assert_eq!(path, missing.display().to_string());
            }
            other => panic!("expected missing path error, got {other:?}"),
        }
    }

    #[test]
    fn permissive_mixed_paths_return_existing_candidates_only() {
        let dir = TestDir::new("permissive-mixed");
        let existing = dir.path.join("input.txt");
        let missing = dir.path.join("missing.txt");
        write_file(&existing);

        let candidates =
            collect_ask_candidates(&[existing.clone(), missing], false, &[], &[], false)
                .expect("missing path should skip");

        assert_eq!(candidates, vec![existing]);
    }

    #[test]
    fn permissive_all_missing_paths_return_empty_candidates() {
        let missing = unique_path("all-missing");

        let candidates = collect_ask_candidates(&[missing], false, &[], &[], false)
            .expect("missing paths should skip");

        assert!(candidates.is_empty());
    }

    #[test]
    fn permissive_missing_path_still_errors_for_dir_without_recursive() {
        let dir = TestDir::new("permissive-dir");
        let missing = unique_path("permissive-dir-missing");

        let error = collect_ask_candidates(&[missing, dir.path.clone()], false, &[], &[], false)
            .expect_err("dir should still fail");

        match error {
            AppError::DirectoryRequiresRecursive { path } => {
                assert_eq!(path, dir.path.display().to_string());
            }
            other => panic!("expected recursive error, got {other:?}"),
        }
    }

    #[test]
    fn no_candidates_return_no_input_files_error() {
        let error = load_ask_files(&[], None).expect_err("empty input should fail");

        assert!(matches!(error, AppError::NoInputFiles));
    }

    #[test]
    fn binary_file_skips_load() {
        let dir = TestDir::new("binary");
        let binary = dir.path.join("input.bin");
        let text = dir.path.join("input.txt");
        write_bytes(&binary, b"bin\0data");
        write_text(&text, "text");

        let loaded = load_ask_files(&[binary, text.clone()], None).expect("load should pass");

        assert_eq!(loaded.total_bytes, 4);
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files[0].path, text);
        assert_eq!(loaded.files[0].content, "text");
    }

    #[test]
    fn non_utf8_file_skips_load() {
        let dir = TestDir::new("utf8");
        let invalid = dir.path.join("bad.txt");
        let text = dir.path.join("good.txt");
        write_bytes(&invalid, &[0x66, 0x6f, 0x80]);
        write_text(&text, "ok");

        let loaded = load_ask_files(&[invalid, text.clone()], None).expect("load should pass");

        assert_eq!(loaded.total_bytes, 2);
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files[0].path, text);
        assert_eq!(loaded.files[0].content, "ok");
    }

    #[test]
    fn binary_only_input_returns_no_input_files_error() {
        let dir = TestDir::new("binary-only");
        let binary = dir.path.join("input.bin");
        write_bytes(&binary, b"bin\0data");

        let error = load_ask_files(&[binary], None).expect_err("binary-only input should fail");

        assert!(matches!(error, AppError::NoInputFiles));
    }

    #[test]
    fn non_utf8_only_input_returns_no_input_files_error() {
        let dir = TestDir::new("utf8-only");
        let invalid = dir.path.join("bad.txt");
        write_bytes(&invalid, &[0x66, 0x6f, 0x80]);

        let error = load_ask_files(&[invalid], None).expect_err("non-utf8 input should fail");

        assert!(matches!(error, AppError::NoInputFiles));
    }

    #[test]
    fn load_counts_total_bytes() {
        let dir = TestDir::new("bytes");
        let alpha = dir.path.join("alpha.txt");
        let beta = dir.path.join("beta.txt");
        write_text(&alpha, "ab");
        write_text(&beta, "cde");

        let loaded =
            load_ask_files(&[alpha.clone(), beta.clone()], None).expect("load should pass");

        assert_eq!(loaded.total_bytes, 5);
        assert_eq!(loaded.files.len(), 2);
        assert_eq!(loaded.files[0].bytes, 2);
        assert_eq!(loaded.files[1].bytes, 3);
    }

    #[test]
    fn load_fails_when_max_bytes_exceeded() {
        let dir = TestDir::new("max-bytes");
        let file = dir.path.join("input.txt");
        write_text(&file, "abc");

        let error = load_ask_files(&[file], Some(2)).expect_err("load should fail");

        match error {
            AppError::MaxBytesExceeded {
                max_bytes,
                actual_bytes,
            } => {
                assert_eq!(max_bytes, 2);
                assert_eq!(actual_bytes, 3);
            }
            other => panic!("expected max-bytes error, got {other:?}"),
        }
    }

    fn write_file(path: &Path) {
        write_text(path, "stub");
    }

    fn write_text(path: &Path, content: &str) {
        let parent = path.parent().expect("file should have parent");
        fs::create_dir_all(parent).expect("parent dir should create");
        fs::write(path, content).expect("file should write");
    }

    fn write_bytes(path: &Path, content: &[u8]) {
        let parent = path.parent().expect("file should have parent");
        fs::create_dir_all(parent).expect("parent dir should create");
        fs::write(path, content).expect("file should write");
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = unique_path(label);
            fs::create_dir_all(&path).expect("dir should create");
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn unique_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();

        env::temp_dir().join(format!("cowork-{label}-{}-{nanos}", process::id()))
    }
}
