use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::error::AppError;

/// Default Ollama host for `ask`.
pub const DEFAULT_HOST: &str = "http://localhost:11434";

/// Resolved config for `ask`.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedAskConfig {
    /// Model name after config merge.
    pub model: Option<String>,
    /// Host after config merge.
    pub host: String,
    /// Config files read during merge.
    pub loaded_files: Vec<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
/// Raw file config.
struct FileConfig {
    #[serde(default)]
    /// Ask config block.
    ask: AskConfig,
}

#[derive(Debug, Default, Deserialize)]
/// Raw ask config.
struct AskConfig {
    /// Optional model override.
    model: Option<String>,
    /// Optional host override.
    host: Option<String>,
}

/// Resolve `ask` config from user config, project config, then CLI.
///
/// # Errors
///
/// Returns [`AppError`] when a config file cannot be read or parsed.
pub fn resolve_ask_config(
    project_dir: &Path,
    home_dir: Option<&Path>,
    cli_model: Option<String>,
    cli_host: Option<String>,
) -> Result<ResolvedAskConfig, AppError> {
    let mut loaded_files = Vec::new();
    let user_path = home_dir.map(|home| home.join(".cowork/config.toml"));
    let project_path = project_dir.join("cowork.toml");
    let user_config = user_path
        .as_deref()
        .map(|path| load_optional_ask_config(path, &mut loaded_files))
        .transpose()?
        .unwrap_or_default();
    let project_config = load_optional_ask_config(&project_path, &mut loaded_files)?;

    Ok(ResolvedAskConfig {
        model: cli_model.or(project_config.model).or(user_config.model),
        host: cli_host
            .or(project_config.host)
            .or(user_config.host)
            .unwrap_or_else(|| DEFAULT_HOST.to_string()),
        loaded_files,
    })
}

/// Load one optional ask config file.
///
/// # Errors
///
/// Returns [`AppError`] when the config file cannot be read or parsed.
fn load_optional_ask_config(
    path: &Path,
    loaded_files: &mut Vec<PathBuf>,
) -> Result<AskConfig, AppError> {
    if !path.exists() {
        return Ok(AskConfig::default());
    }

    let config = fs::read_to_string(path).map_err(|error| {
        AppError::invalid_arguments(format!(
            "failed to read config file {}: {error}",
            path.display()
        ))
    })?;
    let parsed = toml::from_str::<FileConfig>(&config).map_err(|error| {
        AppError::invalid_arguments(format!(
            "failed to parse config file {}: {error}",
            path.display()
        ))
    })?;
    loaded_files.push(path.to_path_buf());

    Ok(parsed.ask)
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

    use super::{DEFAULT_HOST, resolve_ask_config};

    #[test]
    fn missing_config_files_are_noop() {
        let dirs = test_dirs();

        let config = resolve_ask_config(&dirs.project, Some(&dirs.home), None, None)
            .expect("missing config should be ok");

        assert_eq!(config.model, None);
        assert_eq!(config.host, DEFAULT_HOST);
        assert!(config.loaded_files.is_empty());
    }

    #[test]
    fn project_config_beats_user_config() {
        let dirs = test_dirs();
        write_config(
            &dirs.home.join(".cowork/config.toml"),
            "[ask]\nmodel = \"user-model\"\nhost = \"http://user-host\"\n",
        );
        write_config(
            &dirs.project.join("cowork.toml"),
            "[ask]\nmodel = \"project-model\"\nhost = \"http://project-host\"\n",
        );

        let config = resolve_ask_config(&dirs.project, Some(&dirs.home), None, None)
            .expect("config should load");

        assert_eq!(config.model.as_deref(), Some("project-model"));
        assert_eq!(config.host, "http://project-host");
        assert_eq!(
            config.loaded_files,
            vec![
                dirs.home.join(".cowork/config.toml"),
                dirs.project.join("cowork.toml")
            ]
        );
    }

    #[test]
    fn missing_home_skips_user_config() {
        let dirs = test_dirs();
        write_config(
            &dirs.home.join(".cowork/config.toml"),
            "[ask]\nmodel = \"user-model\"\nhost = \"http://user-host\"\n",
        );

        let config =
            resolve_ask_config(&dirs.project, None, None, None).expect("config should load");

        assert_eq!(config.model, None);
        assert_eq!(config.host, DEFAULT_HOST);
        assert!(config.loaded_files.is_empty());
    }

    #[test]
    fn cli_flags_beat_config() {
        let dirs = test_dirs();
        write_config(
            &dirs.home.join(".cowork/config.toml"),
            "[ask]\nmodel = \"user-model\"\nhost = \"http://user-host\"\n",
        );
        write_config(
            &dirs.project.join("cowork.toml"),
            "[ask]\nmodel = \"project-model\"\nhost = \"http://project-host\"\n",
        );

        let config = resolve_ask_config(
            &dirs.project,
            Some(&dirs.home),
            Some("cli-model".to_string()),
            Some("http://cli-host".to_string()),
        )
        .expect("config should load");

        assert_eq!(config.model.as_deref(), Some("cli-model"));
        assert_eq!(config.host, "http://cli-host");
        assert_eq!(
            config.loaded_files,
            vec![
                dirs.home.join(".cowork/config.toml"),
                dirs.project.join("cowork.toml")
            ]
        );
    }

    struct TestDirs {
        project: PathBuf,
        home: PathBuf,
    }

    fn test_dirs() -> TestDirs {
        let root = std::env::temp_dir().join(format!(
            "cowork-config-{}-{}",
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
