use std::{
    fs,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::{DirEntry, WalkDir};

use crate::error::AppError;

const DEFAULT_EXCLUDED_DIRS: [&str; 8] = [
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".next",
    ".cache",
    "coverage",
];

pub fn validate_ask_paths(paths: &[PathBuf], recursive: bool) -> Result<(), AppError> {
    for path in paths {
        validate_path(path, recursive)?;
    }

    Ok(())
}

pub fn collect_ask_candidates(
    paths: &[PathBuf],
    recursive: bool,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<PathBuf>, AppError> {
    validate_ask_paths(paths, recursive)?;

    let filters = CandidateFilters::build(include, exclude)?;
    let mut candidates = Vec::new();

    for path in paths {
        collect_path_candidates(path, &filters, &mut candidates);
    }

    candidates.sort();
    candidates.dedup();

    Ok(candidates)
}

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

fn validate_path(path: &Path, recursive: bool) -> Result<(), AppError> {
    if !path.exists() {
        return Err(AppError::missing_path(path));
    }

    if path.is_symlink() {
        return Ok(());
    }

    if path.is_dir() && !recursive {
        return Err(AppError::directory_requires_recursive(path));
    }

    Ok(())
}

fn collect_path_candidates(path: &Path, filters: &CandidateFilters, candidates: &mut Vec<PathBuf>) {
    if path.is_symlink() {
        return;
    }

    if path.is_file() {
        if filters.matches_explicit_file(path) {
            candidates.push(path.to_path_buf());
        }
        return;
    }

    if !path.is_dir() {
        return;
    }

    let walker = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_prune_dir(entry));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if entry.depth() == 0 || entry.file_type().is_dir() || entry.file_type().is_symlink() {
            continue;
        }

        let entry_path = entry.into_path();
        if filters.matches_discovered_file(&entry_path) {
            candidates.push(entry_path);
        }
    }
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

fn should_prune_dir(entry: &DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return false;
    }

    let Some(name) = entry.file_name().to_str() else {
        return false;
    };

    DEFAULT_EXCLUDED_DIRS.contains(&name)
}

struct CandidateFilters {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

impl CandidateFilters {
    fn build(include: &[String], exclude: &[String]) -> Result<Self, AppError> {
        Ok(Self {
            include: compile_globs(include, "--include")?,
            exclude: compile_globs(exclude, "--exclude")?,
        })
    }

    fn matches_explicit_file(&self, path: &Path) -> bool {
        !self.is_excluded(path)
    }

    fn matches_discovered_file(&self, path: &Path) -> bool {
        self.is_included(path) && !self.is_excluded(path)
    }

    fn is_included(&self, path: &Path) -> bool {
        match &self.include {
            Some(include) => matches_glob(include, path),
            None => true,
        }
    }

    fn is_excluded(&self, path: &Path) -> bool {
        match &self.exclude {
            Some(exclude) => matches_glob(exclude, path),
            None => false,
        }
    }
}

fn compile_globs(globs: &[String], flag: &str) -> Result<Option<GlobSet>, AppError> {
    if globs.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        let parsed = Glob::new(glob).map_err(|error| {
            AppError::invalid_arguments(format!("invalid {flag} glob `{glob}`: {error}"))
        })?;
        builder.add(parsed);
    }

    builder
        .build()
        .map(Some)
        .map_err(|error| AppError::invalid_arguments(format!("invalid {flag} globs: {error}")))
}

fn matches_glob(globs: &GlobSet, path: &Path) -> bool {
    globs.is_match(path)
        || path
            .file_name()
            .is_some_and(|file_name| globs.is_match(Path::new(file_name)))
}

fn is_binary_file(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::error::AppError;

    use super::{collect_ask_candidates, load_ask_files, validate_ask_paths};

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
    fn recursive_dir_walk_collects_files() {
        let dir = TestDir::new("recursive");
        let alpha = dir.path.join("src/alpha.rs");
        let beta = dir.path.join("src/nested/beta.txt");
        write_file(&alpha);
        write_file(&beta);

        let candidates = collect_ask_candidates(std::slice::from_ref(&dir.path), true, &[], &[])
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

        let candidates = collect_ask_candidates(std::slice::from_ref(&dir.path), true, &[], &[])
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
        )
        .expect("explicit file should pass");

        assert_eq!(candidates, vec![file]);
    }

    #[test]
    fn file_path_passes_gate() {
        let dir = TestDir::new("file-dir");
        let file = dir.path.join("input.txt");
        write_file(&file);

        validate_ask_paths(&[file], false).expect("file path should pass");
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
