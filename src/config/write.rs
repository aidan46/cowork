use std::{fs, io, path::Path, path::PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Default, Deserialize)]
/// Raw file config needed for write policy.
struct FileConfig {
    #[serde(default)]
    /// Ask config block.
    ask: AskConfig,
}

#[derive(Debug, Default, Deserialize)]
/// Raw ask config needed for write policy.
struct AskConfig {
    /// Optional model override.
    model: Option<String>,
    /// Optional host override.
    host: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
/// Ask config with source spans.
struct SpannedAskConfig {
    /// Optional model value and source span.
    model: Option<toml::Spanned<String>>,
    /// Optional host value and source span.
    host: Option<toml::Spanned<String>>,
}

#[derive(Debug, Default, Deserialize)]
/// File config with spanned ask values.
struct SpannedFileConfig {
    #[serde(default)]
    /// Ask config block.
    ask: SpannedAskConfig,
}

#[derive(Debug, PartialEq, Eq)]
/// Result of requested ask config write.
pub(crate) enum AskConfigWrite {
    /// Config bytes changed.
    Written,
    /// Config already held requested values.
    Unchanged,
}

#[derive(Debug, Error)]
/// Ask config write failure.
pub(crate) enum AskConfigWriteError {
    #[error("failed to read config file {path}: {source}")]
    /// Existing config read failed.
    Read {
        /// Config path.
        path: PathBuf,
        /// IO failure.
        source: io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    /// Existing config parse failed.
    Parse {
        /// Config path.
        path: PathBuf,
        /// TOML parse failure.
        source: toml::de::Error,
    },
    #[error("existing `[ask].{key}` differs; use `--force` to replace it")]
    /// Existing ask value conflicts with requested value.
    Conflict {
        /// Conflicting ask key.
        key: &'static str,
    },
    #[error("existing `[ask]` layout cannot accept missing keys without rewriting it")]
    /// Existing non-table ask layout cannot accept missing keys narrowly.
    UnsupportedLayout,
    #[error("failed to create config directory {path}: {source}")]
    /// Config parent creation failed.
    CreateDir {
        /// Parent path.
        path: PathBuf,
        /// IO failure.
        source: io::Error,
    },
    #[error("failed to write config file {path}: {source}")]
    /// Config file write failed.
    Write {
        /// Config path.
        path: PathBuf,
        /// IO failure.
        source: io::Error,
    },
}

/// Write chosen ask model and host while preserving unrelated config bytes.
///
/// # Errors
///
/// Returns [`AskConfigWriteError`] when config cannot be read, merged, or written.
pub(crate) fn write_ask_config(
    path: &Path,
    model: &str,
    host: &str,
    force: bool,
) -> Result<AskConfigWrite, AskConfigWriteError> {
    let original = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(AskConfigWriteError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let parsed =
        toml::from_str::<FileConfig>(&original).map_err(|source| AskConfigWriteError::Parse {
            path: path.to_path_buf(),
            source,
        })?;

    reject_conflict("model", parsed.ask.model.as_deref(), model, force)?;
    reject_conflict("host", parsed.ask.host.as_deref(), host, force)?;

    let model_changed = parsed.ask.model.as_deref() != Some(model);
    let host_changed = parsed.ask.host.as_deref() != Some(host);
    if !model_changed && !host_changed {
        return Ok(AskConfigWrite::Unchanged);
    }

    let merged = merge_ask_config(
        path,
        &original,
        model,
        host,
        parsed.ask.model.is_some(),
        parsed.ask.host.is_some(),
    )?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| AskConfigWriteError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, merged).map_err(|source| AskConfigWriteError::Write {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(AskConfigWrite::Written)
}

/// Reject changed existing value without force.
///
/// # Errors
///
/// Returns conflict when existing value differs without force.
fn reject_conflict(
    key: &'static str,
    existing: Option<&str>,
    requested: &str,
    force: bool,
) -> Result<(), AskConfigWriteError> {
    if !force && existing.is_some_and(|value| value != requested) {
        return Err(AskConfigWriteError::Conflict { key });
    }

    Ok(())
}

/// Merge ask values into original TOML text.
///
/// # Errors
///
/// Returns parse or layout failure when narrow merge is unavailable.
fn merge_ask_config(
    path: &Path,
    original: &str,
    model: &str,
    host: &str,
    has_model: bool,
    has_host: bool,
) -> Result<String, AskConfigWriteError> {
    let spanned = toml::from_str::<SpannedFileConfig>(original).map_err(|source| {
        AskConfigWriteError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let mut replacements = Vec::new();
    if let Some(existing) = spanned.ask.model
        && existing.get_ref() != model
    {
        replacements.push((existing.span(), toml_string(model)));
    }
    if let Some(existing) = spanned.ask.host
        && existing.get_ref() != host
    {
        replacements.push((existing.span(), toml_string(host)));
    }

    replacements.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut merged = original.to_string();
    for (span, value) in replacements {
        merged.replace_range(span, &value);
    }

    insert_missing_ask_keys(path, &mut merged, model, host, !has_model, !has_host)?;
    Ok(merged)
}

/// Render one TOML string value.
fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

/// Insert missing ask keys into explicit ask table or new table.
///
/// # Errors
///
/// Returns parse or layout failure when insertion cannot preserve config.
fn insert_missing_ask_keys(
    path: &Path,
    contents: &mut String,
    model: &str,
    host: &str,
    add_model: bool,
    add_host: bool,
) -> Result<(), AskConfigWriteError> {
    if !add_model && !add_host {
        return Ok(());
    }

    let mut rows = String::new();
    if add_model {
        rows.push_str(&format!("model = {}\n", toml_string(model)));
    }
    if add_host {
        rows.push_str(&format!("host = {}\n", toml_string(host)));
    }

    if let Some(section_end) = ask_section_end(contents) {
        if section_end > 0 && !contents[..section_end].ends_with('\n') {
            rows.insert(0, '\n');
        }
        contents.insert_str(section_end, &rows);
        return Ok(());
    }

    let root =
        toml::from_str::<toml::Table>(contents).map_err(|source| AskConfigWriteError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    if root.contains_key("ask") {
        return Err(AskConfigWriteError::UnsupportedLayout);
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    if !contents.is_empty() && !contents.ends_with("\n\n") {
        contents.push('\n');
    }
    contents.push_str("[ask]\n");
    contents.push_str(&rows);

    Ok(())
}

/// Find byte offset ending explicit ask table.
fn ask_section_end(contents: &str) -> Option<usize> {
    let mut offset = 0;
    let mut in_ask = false;

    for line in contents.split_inclusive('\n') {
        let trimmed = line.trim();
        if is_ask_header(trimmed) {
            in_ask = true;
        } else if in_ask && trimmed.starts_with('[') {
            return Some(offset);
        }
        offset += line.len();
    }

    in_ask.then_some(contents.len())
}

/// Match ask header with optional trailing comment.
fn is_ask_header(line: &str) -> bool {
    line.strip_prefix("[ask]").is_some_and(|tail| {
        let tail = tail.trim_start();
        tail.is_empty() || tail.starts_with('#')
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{AskConfigWrite, AskConfigWriteError, write_ask_config};

    #[test]
    fn creates_parent_and_file() {
        let dirs = test_dirs();
        let path = dirs.home.join(".cowork/config.toml");

        let result = write_ask_config(&path, "qwen2.5-coder:7b", "http://localhost:11434", false)
            .expect("config should write");

        assert_eq!(result, AskConfigWrite::Written);
        assert_eq!(
            fs::read_to_string(path).expect("config should read"),
            concat!(
                "[ask]\n",
                "model = \"qwen2.5-coder:7b\"\n",
                "host = \"http://localhost:11434\"\n"
            )
        );
    }

    #[test]
    fn preserves_unrelated_toml() {
        let dirs = test_dirs();
        let path = dirs.project.join("cowork.toml");
        let original = concat!(
            "# keep comment\n",
            "title = \"unchanged\"\n\n",
            "[ask] # keep ask comment\n",
            "model = \"qwen2.5-coder:7b\"\n\n",
            "[other]\n",
            "enabled = true\n"
        );
        write_config(&path, original);

        let result = write_ask_config(&path, "qwen2.5-coder:7b", "http://localhost:11434", false)
            .expect("missing host should merge");
        let written = fs::read_to_string(path).expect("config should read");

        assert_eq!(result, AskConfigWrite::Written);
        assert!(written.starts_with("# keep comment\ntitle = \"unchanged\"\n\n"));
        assert!(written.contains(
            "model = \"qwen2.5-coder:7b\"\n\nhost = \"http://localhost:11434\"\n[other]"
        ));
        assert!(written.ends_with("[other]\nenabled = true\n"));
    }

    #[test]
    fn equal_values_are_unchanged() {
        let dirs = test_dirs();
        let path = dirs.project.join("cowork.toml");
        let original = "[ask]\nmodel = \"same\"\nhost = \"http://same\"\n";
        write_config(&path, original);

        let result = write_ask_config(&path, "same", "http://same", false)
            .expect("equal values should pass");

        assert_eq!(result, AskConfigWrite::Unchanged);
        assert_eq!(
            fs::read_to_string(path).expect("config should read"),
            original
        );
    }

    #[test]
    fn rejects_conflict_without_changes() {
        let dirs = test_dirs();
        let path = dirs.project.join("cowork.toml");
        let original = "[ask]\nmodel = \"old\"\n\n[other]\nkeep = true\n";
        write_config(&path, original);

        let error = write_ask_config(&path, "new", "http://new", false)
            .expect_err("different model should conflict");

        assert!(matches!(
            error,
            AskConfigWriteError::Conflict { key: "model" }
        ));
        assert_eq!(
            fs::read_to_string(path).expect("config should read"),
            original
        );
    }

    #[test]
    fn force_replaces_ask_values_only() {
        let dirs = test_dirs();
        let path = dirs.project.join("cowork.toml");
        let original = concat!(
            "title = \"keep\"\n\n",
            "[ask]\n",
            "model = 'old-model' # keep model comment\n",
            "host = \"http://old\"\n\n",
            "[other]\n",
            "keep = true\n"
        );
        write_config(&path, original);

        let result = write_ask_config(&path, "new-model", "http://new", true)
            .expect("force should replace values");
        let written = fs::read_to_string(path).expect("config should read");

        assert_eq!(result, AskConfigWrite::Written);
        assert!(written.starts_with("title = \"keep\"\n\n[ask]\n"));
        assert!(written.contains("model = \"new-model\" # keep model comment\n"));
        assert!(written.contains("host = \"http://new\"\n\n[other]\n"));
        assert!(written.ends_with("keep = true\n"));
    }

    struct TestDirs {
        project: PathBuf,
        home: PathBuf,
    }

    fn test_dirs() -> TestDirs {
        let root = std::env::temp_dir().join(format!(
            "cowork-config-write-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        ));
        let project = root.join("project");
        let home = root.join("home");

        fs::create_dir_all(&project).expect("project dir should create");
        fs::create_dir_all(&home).expect("home dir should create");

        TestDirs { project, home }
    }

    fn write_config(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir should create");
        }

        fs::write(path, contents).expect("config should write");
    }
}
