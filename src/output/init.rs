use super::{CLAUDE_INIT_RULES, CODEX_INIT_RULES};

#[must_use]
/// Return init rules for agent.
///
/// # Panics
///
/// Panics when `agent` is not a supported init target.
pub(crate) fn render_init_rules(agent: &str) -> &'static str {
    match agent {
        "codex" => CODEX_INIT_RULES,
        "claude" => CLAUDE_INIT_RULES,
        _ => unreachable!("unsupported init agent"),
    }
}

#[must_use]
/// Return target file for agent init.
///
/// # Panics
///
/// Panics when `agent` is not a supported init target.
pub(crate) fn init_target_file(agent: &str) -> &'static str {
    match agent {
        "codex" => "AGENTS.md",
        "claude" => "CLAUDE.md",
        _ => unreachable!("unsupported init agent"),
    }
}

#[must_use]
/// Render managed init block.
///
/// # Panics
///
/// Panics when `agent` is not a supported init target.
pub(crate) fn render_init_managed_block(agent: &str) -> String {
    let (start, end) = init_block_markers(agent);

    format!("{start}\n{}{end}\n", render_init_rules(agent))
}

/// Replace or append managed init block.
///
/// # Errors
///
/// Returns an error when one managed marker exists without the other.
pub(crate) fn update_init_managed_block(
    agent: &str,
    current: &str,
) -> Result<String, &'static str> {
    let block = render_init_managed_block(agent);
    let (start, end) = init_block_markers(agent);
    let start_idx = current.find(&start);

    match start_idx {
        Some(start_idx) => {
            let end_rel = current[start_idx..]
                .find(&end)
                .ok_or("managed block end marker missing")?;
            let mut end_idx = start_idx + end_rel + end.len();

            if current[end_idx..].starts_with('\n') {
                end_idx += 1;
            }
            let mut next =
                String::with_capacity(current.len() - (end_idx - start_idx) + block.len());

            next.push_str(&current[..start_idx]);
            next.push_str(&block);
            next.push_str(&current[end_idx..]);

            Ok(next)
        }
        None if current.contains(&end) => Err("managed block start marker missing"),
        None if current.is_empty() => Ok(block),
        None => {
            let sep = if current.ends_with("\n\n") {
                ""
            } else if current.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };

            Ok(format!("{current}{sep}{block}"))
        }
    }
}

#[must_use]
/// Build init block markers.
fn init_block_markers(agent: &str) -> (String, String) {
    (
        format!("<!-- cowork:init:start agent={agent} -->"),
        format!("<!-- cowork:init:end agent={agent} -->"),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use super::{render_init_managed_block, render_init_rules, update_init_managed_block};

    #[test]
    fn codex_init_rules_keep_required_markers() {
        let rules = render_init_rules("codex");

        assert!(rules.contains("# cowork rules for Codex"));
        assert!(rules.contains("cowork ask"));
        assert!(rules.contains("cowork doctor"));
        assert!(rules.contains("next_reads"));
        assert!(rules.contains("lead, not authority"));
    }

    #[test]
    fn claude_init_rules_keep_required_markers() {
        let rules = render_init_rules("claude");

        assert!(rules.contains("# cowork rules for Claude"));
        assert!(rules.contains("cowork ask"));
        assert!(rules.contains("cowork doctor"));
        assert!(rules.contains("next_reads"));
        assert!(rules.contains("lead, not authority"));
    }

    #[test]
    fn codex_managed_block_keeps_required_markers() {
        let block = render_init_managed_block("codex");

        assert!(block.contains("<!-- cowork:init:start agent=codex -->"));
        assert!(block.contains("<!-- cowork:init:end agent=codex -->"));
        assert!(block.contains("# cowork rules for Codex"));
    }

    #[test]
    fn existing_managed_block_replaces_only_block_body() {
        let updated = update_init_managed_block(
            "codex",
            "before\n\n<!-- cowork:init:start agent=codex -->\nold\n<!-- cowork:init:end agent=codex -->\n\nafter\n",
        )
        .expect("managed block should update");

        assert!(updated.starts_with("before\n\n"));
        assert!(updated.ends_with("\n\nafter\n"));
        assert!(!updated.contains("\nold\n"));
        assert_eq!(
            updated
                .matches("<!-- cowork:init:start agent=codex -->")
                .count(),
            1
        );
    }
}
