use crate::{Framework, model::SourceTask, paths};
use anyhow::{Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path, sync::OnceLock};

fn checkbox() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[([\sxX])\]\s*(.*)").unwrap())
}
fn task_id() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^T[0-9]+\b").unwrap())
}
fn story() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[(US[0-9]+)\]").unwrap())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub source_path: String,
    pub source_hash: String,
    pub parser_profile: String,
    pub frontmatter: serde_json::Value,
    pub headings: Vec<Heading>,
    pub tasks: Vec<SourceTask>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub line: usize,
}
/// The ATX heading level of a trimmed line (`#` run followed by a space).
fn heading_level(stripped: &str) -> Option<usize> {
    let level = stripped.chars().take_while(|c| *c == '#').count();
    (level > 0 && stripped.chars().nth(level) == Some(' ')).then_some(level)
}
/// Leading whitespace width; a tab counts as four columns.
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// Lines collected under the most recent task checkbox.
struct Details {
    task: usize,
    indent: usize,
    /// Collected lines; `true` marks fence bodies, which never set the shared indent.
    lines: Vec<(String, bool)>,
    blanks: usize,
    fenced: bool,
}
impl Details {
    fn finish(self, tasks: &mut [SourceTask]) {
        let common = self
            .lines
            .iter()
            .filter(|(l, verbatim)| !verbatim && !l.trim().is_empty())
            .map(|(l, _)| indent(l))
            .min()
            .unwrap_or(0);
        let body: Vec<String> = self
            .lines
            .iter()
            .map(|(line, _)| {
                // Drop the shared indentation only; relative layout and code stay as written.
                let mut width = 0;
                let cut = line
                    .char_indices()
                    .find(|(_, c)| {
                        if width >= common || !c.is_whitespace() {
                            return true;
                        }
                        width += if *c == '\t' { 4 } else { 1 };
                        false
                    })
                    .map_or(line.len(), |(i, _)| i);
                line[cut..].to_string()
            })
            .collect();
        tasks[self.task].details = body.join("\n");
    }
}

pub fn parse(path: &str, text: &str, framework: Framework) -> Result<Document> {
    let hash = paths::hash(text);
    let mut doc = Document {
        source_path: path.into(),
        source_hash: hash.clone(),
        parser_profile: format!("{:?}-v1", framework).to_lowercase(),
        frontmatter: serde_json::Value::Object(Default::default()),
        headings: vec![],
        tasks: vec![],
        diagnostics: vec![],
    };
    let mut phase = String::new();
    let mut offset = 0;
    let mut fence: Option<(char, usize)> = None;
    let mut comment = false;
    let mut ids = BTreeSet::new();
    let mut front = String::new();
    let mut front_kind: Option<&str> = None;
    let mut details: Option<Details> = None;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let stripped = line.trim();
        if index == 0 && ["---", "+++"].contains(&stripped) {
            front_kind = Some(if stripped == "---" { "yaml" } else { "toml" });
            offset += line.len();
            continue;
        }
        if let Some(kind) = front_kind {
            if stripped == if kind == "yaml" { "---" } else { "+++" } {
                doc.frontmatter = if kind == "yaml" {
                    serde_yaml::from_str::<serde_json::Value>(&front)?
                } else {
                    serde_json::to_value(toml::from_str::<toml::Value>(&front)?)?
                };
                front_kind = None;
            } else {
                front.push_str(line);
            }
            offset += line.len();
            continue;
        }
        let fence_line = stripped.starts_with("```") || stripped.starts_with("~~~");
        let fenced_before = fence.is_some();
        if fence_line {
            let ch = stripped.chars().next().unwrap();
            let count = stripped.chars().take_while(|c| *c == ch).count();
            if let Some((open, len)) = fence {
                if open == ch && count >= len && stripped[count..].trim().is_empty() {
                    fence = None;
                }
            } else {
                fence = Some((ch, count));
            }
        }
        let hidden = fence.is_some() || comment || stripped.starts_with("<!--");
        let content = line.trim_end_matches(['\r', '\n']);
        // Any checkbox the parser reads (a task or an empty_task diagnostic) ends details.
        let task_line =
            checkbox().is_match(content) && (!hidden || framework == Framework::Openspec);
        // Indented lines under a checkbox belong to it until the next task, a heading or
        // a line indented no deeper than the checkbox. A fence opened inside is kept
        // whole, whatever its own lines look like, unless one of them is itself a task.
        if let Some(mut open) = details.take() {
            let mut verbatim = false;
            let keep = if task_line {
                false
            } else if open.fenced {
                open.fenced = fence.is_some();
                verbatim = true;
                true
            } else if stripped.is_empty() {
                open.blanks += 1;
                details = Some(open);
                offset += line.len();
                continue_after_details(&mut comment, stripped);
                continue;
            } else if !hidden && heading_level(stripped).is_some() {
                false
            } else if indent(content) > open.indent {
                open.fenced = fence_line && !fenced_before && fence.is_some();
                true
            } else {
                false
            };
            if keep {
                open.lines
                    .extend(std::iter::repeat_n((String::new(), verbatim), open.blanks));
                open.blanks = 0;
                open.lines.push((content.to_string(), verbatim));
                details = Some(open);
                offset += line.len();
                continue_after_details(&mut comment, stripped);
                continue;
            }
            open.finish(&mut doc.tasks);
        }
        if !hidden && let Some(level) = heading_level(stripped) {
            let title = stripped[level..].trim().to_string();
            if level == 2 {
                phase = title.clone();
            }
            doc.headings.push(Heading {
                level,
                text: title,
                line: index + 1,
            });
        }
        if let Some(c) = checkbox().captures(line.trim_end_matches(['\r', '\n'])) {
            if !hidden || framework == Framework::Openspec {
                let desc = c.get(2).unwrap().as_str().trim();
                if desc.is_empty() {
                    doc.diagnostics.push(format!("empty_task:{}", index + 1));
                } else {
                    if hidden {
                        doc.diagnostics
                            .push(format!("upstream_counts_hidden_checkbox:{}", index + 1));
                    }
                    let id = match framework {
                        Framework::Speckit => {
                            if let Some(m) = task_id().find(desc) {
                                m.as_str().into()
                            } else {
                                doc.diagnostics
                                    .push(format!("task_id_missing:{}", index + 1));
                                format!("md-{}", &paths::hash(desc)[..16])
                            }
                        }
                        Framework::Openspec => format!("os-{}", &paths::hash(desc)[..16]),
                    };
                    if !ids.insert(id.clone()) {
                        bail!(
                            "ambiguous_source_task: duplicate task identity at line {}",
                            index + 1
                        );
                    }
                    doc.tasks.push(SourceTask {
                        id,
                        description: desc.into(),
                        done: c.get(1).unwrap().as_str().eq_ignore_ascii_case("x"),
                        source_path: path.into(),
                        source_hash: hash.clone(),
                        text_hash: paths::hash(desc),
                        line: index + 1,
                        checkbox_byte: offset + c.get(1).unwrap().start(),
                        phase: phase.clone(),
                        story: story().captures(desc).map(|c| c[1].into()),
                        parallel: desc.contains("[P]"),
                        details: String::new(),
                    });
                    details = Some(Details {
                        task: doc.tasks.len() - 1,
                        indent: indent(content),
                        lines: vec![],
                        blanks: 0,
                        fenced: false,
                    });
                }
            }
        }
        if stripped.contains("<!--") {
            comment = true;
        }
        if stripped.contains("-->") {
            comment = false;
        }
        offset += line.len();
    }
    if let Some(open) = details {
        open.finish(&mut doc.tasks);
    }
    if front_kind.is_some() {
        bail!("invalid_markdown: unclosed frontmatter");
    }
    Ok(doc)
}
/// Comment state tracking for lines consumed as task details.
fn continue_after_details(comment: &mut bool, stripped: &str) {
    if stripped.contains("<!--") {
        *comment = true;
    }
    if stripped.contains("-->") {
        *comment = false;
    }
}
pub fn complete(
    root: &Path,
    path: &str,
    framework: Framework,
    expected_hash: &str,
    ids: &[String],
) -> Result<()> {
    let text = paths::read(root, path, 2 * 1024 * 1024)?;
    if paths::hash(&text) != expected_hash {
        bail!("source_drift: tracking file changed before writeback");
    }
    let parsed = parse(path, &text, framework)?;
    let mut replacements = vec![];
    for id in ids {
        let Some(task) = parsed.tasks.iter().find(|t| &t.id == id) else {
            bail!("source_drift: source task {id} is missing");
        };
        replacements.push((
            task.checkbox_byte,
            text[task.checkbox_byte..]
                .chars()
                .next()
                .unwrap()
                .len_utf8(),
        ));
    }
    replacements.sort_unstable();
    replacements.dedup();
    let mut bytes = text.into_bytes();
    for (offset, len) in replacements.into_iter().rev() {
        bytes.splice(offset..offset + len, b"x".iter().copied());
    }
    paths::atomic_write(&paths::inside(root, path)?, bytes)
}
