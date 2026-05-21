use crate::files::{LoadedAskFile, LoadedAskFiles};

/// Prompt role line.
const ROLE_LINE: &str = "You are local coding coworker helping another coding agent inspect files.";
/// Shared hard rules.
const HARD_RULES: [&str; 7] = [
    "Use only provided files.",
    "Return valid JSON only.",
    "No Markdown.",
    "No prose outside JSON.",
    "No comments.",
    "Do not speculate beyond provided files.",
    "Keep next_reads inside provided paths or obvious adjacent files only.",
];
/// Ask schema block.
const ASK_SCHEMA_BLOCK: &str = concat!(
    "schema_version:string\n",
    "command:\"ask\"\n",
    "status:\"ok\"\n",
    "question:string\n",
    "answer:{summary:string,confidence:\"high\"|\"medium\"|\"low\"|\"unknown\",not_found:boolean}\n",
    "files:[{path:string,included:boolean,reason:string,bytes:number}]\n",
    "symbols:[{name:string,kind:\"function\"|\"type\"|\"trait\"|\"impl\"|\"module\"|\"constant\"|\"variable\"|\"route\"|\"component\"|\"test\"|\"unknown\",path:string,relevance:string}]\n",
    "evidence:[{path:string,symbol:string,note:string}]\n",
    "risks:[{kind:\"missing_context\"|\"model_uncertainty\"|\"parse_error\"|\"skipped_file\"|\"unsupported_file\"|\"unknown\",message:string}]\n",
    "next_reads:[{path:string,reason:string}]\n",
    "metadata:{input_bytes:number,duration_ms:number}"
);
/// Locate schema block.
const LOCATE_SCHEMA_BLOCK: &str = concat!(
    "schema_version:string\n",
    "command:\"locate\"\n",
    "status:\"ok\"\n",
    "matches:[{path:string,symbol?:string,kind?:\"function\"|\"type\"|\"trait\"|\"impl\"|\"module\"|\"constant\"|\"variable\"|\"route\"|\"component\"|\"test\"|\"unknown\",reason:string,confidence:\"high\"|\"medium\"|\"low\"|\"unknown\"}]\n",
    "next_reads:[{path:string,reason:string}]\n",
    "risks:[{kind:\"missing_context\"|\"model_uncertainty\"|\"parse_error\"|\"skipped_file\"|\"unsupported_file\"|\"unknown\",message:string}]"
);

/// Render ask prompt text.
pub(crate) fn render_ask_prompt(question: &str, loaded_files: &LoadedAskFiles) -> String {
    let mut prompt = String::new();

    push_line(&mut prompt, ROLE_LINE);
    for rule in HARD_RULES {
        push_line(&mut prompt, rule);
    }
    push_line(
        &mut prompt,
        "If evidence missing, set answer.not_found = true.",
    );
    push_line(&mut prompt, "Keep output concise.");
    prompt.push('\n');
    push_line(&mut prompt, "Question:");
    push_line(&mut prompt, question);
    prompt.push('\n');
    push_line(&mut prompt, "Schema:");
    push_line(&mut prompt, ASK_SCHEMA_BLOCK);
    prompt.push('\n');
    push_line(&mut prompt, "Files:");

    for file in &loaded_files.files {
        render_file_block(&mut prompt, file);
    }

    prompt
}

/// Render locate prompt text.
pub(crate) fn render_locate_prompt(thing: &str, loaded_files: &LoadedAskFiles) -> String {
    let mut prompt = String::new();

    push_line(&mut prompt, ROLE_LINE);
    for rule in HARD_RULES {
        push_line(&mut prompt, rule);
    }
    push_line(&mut prompt, "Find likely files and symbols only.");
    push_line(&mut prompt, "No long explanation.");
    push_line(&mut prompt, "No edit plan.");
    push_line(&mut prompt, "Use empty arrays when unsure.");
    prompt.push('\n');
    push_line(&mut prompt, "Thing:");
    push_line(&mut prompt, thing);
    prompt.push('\n');
    push_line(&mut prompt, "Schema:");
    push_line(&mut prompt, LOCATE_SCHEMA_BLOCK);
    prompt.push('\n');
    push_line(&mut prompt, "Sort matches by confidence, then path.");
    push_line(&mut prompt, "Keep next_reads short and concrete.");
    prompt.push('\n');
    push_line(&mut prompt, "Files:");

    for file in &loaded_files.files {
        render_file_block(&mut prompt, file);
    }

    prompt
}

/// Append one file block.
fn render_file_block(prompt: &mut String, file: &LoadedAskFile) {
    prompt.push_str("<file path=\"");
    prompt.push_str(&file.path.display().to_string());
    prompt.push_str("\">\n");
    prompt.push_str(&file.content);
    if !file.content.ends_with('\n') {
        prompt.push('\n');
    }
    prompt.push_str("</file>\n");
}

/// Push one line and newline.
fn push_line(prompt: &mut String, line: &str) {
    prompt.push_str(line);
    prompt.push('\n');
}

#[cfg(test)]
mod tests {
    #![allow(clippy::missing_errors_doc, reason = "test helpers stay local")]
    #![allow(clippy::missing_panics_doc, reason = "test asserts and fixtures")]

    use std::path::PathBuf;

    use super::{render_ask_prompt, render_locate_prompt};
    use crate::files::{LoadedAskFile, LoadedAskFiles};

    #[test]
    fn prompt_includes_question() {
        let prompt = render_ask_prompt(
            "where auth?",
            &loaded_files([loaded_file("src/lib.rs", "fn main() {}")]),
        );

        assert!(prompt.contains("Question:\nwhere auth?"));
    }

    #[test]
    fn prompt_includes_schema_fields() {
        let prompt = render_ask_prompt("test", &loaded_files([]));

        assert!(prompt.contains("answer:{summary:string,confidence:"));
        assert!(prompt.contains("next_reads:[{path:string,reason:string}]"));
        assert!(prompt.contains("metadata:{input_bytes:number,duration_ms:number}"));
    }

    #[test]
    fn locate_prompt_includes_locate_schema_fields() {
        let prompt = render_locate_prompt("auth middleware", &loaded_files([]));

        assert!(prompt.contains("command:\"locate\""));
        assert!(prompt.contains("matches:[{path:string,symbol?:string"));
        assert!(prompt.contains("risks:[{kind:\"missing_context\""));
    }

    #[test]
    fn prompt_wraps_files_in_tags() {
        let prompt = render_ask_prompt(
            "test",
            &loaded_files([loaded_file("src/lib.rs", "fn main() {}")]),
        );

        assert!(prompt.contains("<file path=\"src/lib.rs\">\nfn main() {}\n</file>"));
    }

    #[test]
    fn prompt_keeps_input_file_order() {
        let prompt = render_ask_prompt(
            "test",
            &loaded_files([
                loaded_file("src/b.rs", "mod b;"),
                loaded_file("src/a.rs", "mod a;"),
            ]),
        );

        let first = prompt.find("<file path=\"src/b.rs\">").expect("first file");
        let second = prompt
            .find("<file path=\"src/a.rs\">")
            .expect("second file");

        assert!(first < second);
    }

    #[test]
    fn locate_prompt_includes_thing() {
        let prompt = render_locate_prompt("auth middleware", &loaded_files([]));

        assert!(prompt.contains("Thing:\nauth middleware"));
    }

    fn loaded_files<const N: usize>(files: [LoadedAskFile; N]) -> LoadedAskFiles {
        LoadedAskFiles {
            total_bytes: files.iter().map(|file| file.bytes).sum(),
            files: files.into(),
        }
    }

    fn loaded_file(path: &str, content: &str) -> LoadedAskFile {
        LoadedAskFile {
            path: PathBuf::from(path),
            content: content.to_owned(),
            bytes: content.len(),
        }
    }
}
