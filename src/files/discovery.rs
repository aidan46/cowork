use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::{DirEntry, WalkDir};

use crate::error::AppError;

/// Dirs skipped during recursive walk.
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

/// Validate ask paths before collect.
pub(crate) fn validate_ask_paths(paths: &[PathBuf], recursive: bool) -> Result<(), AppError> {
    for path in paths {
        validate_path(path, recursive)?;
    }

    Ok(())
}

/// Collect ask candidate files.
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

/// Validate one path input.
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

/// Collect candidates from one path.
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

/// Decide if walk should prune dir.
fn should_prune_dir(entry: &DirEntry) -> bool {
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return false;
    }

    let Some(name) = entry.file_name().to_str() else {
        return false;
    };

    DEFAULT_EXCLUDED_DIRS.contains(&name)
}

/// Include and exclude matchers.
struct CandidateFilters {
    /// Include matcher set.
    include: Option<GlobSet>,
    /// Exclude matcher set.
    exclude: Option<GlobSet>,
}

impl CandidateFilters {
    /// Build filters from CLI globs.
    fn build(include: &[String], exclude: &[String]) -> Result<Self, AppError> {
        Ok(Self {
            include: compile_globs(include, "--include")?,
            exclude: compile_globs(exclude, "--exclude")?,
        })
    }

    /// Check explicit file match.
    fn matches_explicit_file(&self, path: &Path) -> bool {
        !self.is_excluded(path)
    }

    /// Check discovered file match.
    fn matches_discovered_file(&self, path: &Path) -> bool {
        self.is_included(path) && !self.is_excluded(path)
    }

    /// Check include rules.
    fn is_included(&self, path: &Path) -> bool {
        match &self.include {
            Some(include) => matches_glob(include, path),
            None => true,
        }
    }

    /// Check exclude rules.
    fn is_excluded(&self, path: &Path) -> bool {
        match &self.exclude {
            Some(exclude) => matches_glob(exclude, path),
            None => false,
        }
    }
}

/// Compile glob list into matcher.
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

/// Match glob set on path or file name.
fn matches_glob(globs: &GlobSet, path: &Path) -> bool {
    globs.is_match(path)
        || path
            .file_name()
            .is_some_and(|file_name| globs.is_match(Path::new(file_name)))
}
