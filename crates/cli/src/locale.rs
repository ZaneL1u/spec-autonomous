use clap::{CommandFactory, FromArgMatches};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    ZhCn,
}

impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }
    pub fn is_zh(self) -> bool {
        matches!(self, Self::ZhCn)
    }
}

fn zh(value: &str) -> bool {
    let value = value
        .trim()
        .split(':')
        .next()
        .unwrap_or_default()
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace('_', "-");
    value == "zh" || value.starts_with("zh-") || value.starts_with("cmn")
}

pub fn detect(explicit: Option<&str>) -> Locale {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let selected = explicit
        .map(str::to_owned)
        .or_else(|| {
            ["SPEC_AUTONOMOUS_LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE"]
                .into_iter()
                .find_map(env)
        })
        .or_else(system_language)
        .or_else(|| env("LANG"));
    if selected.as_deref().is_some_and(zh) {
        Locale::ZhCn
    } else {
        Locale::En
    }
}

fn system_language() -> Option<String> {
    use std::process::{Command, Stdio};
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("/usr/bin/defaults", &["read", "-g", "AppleLanguages"])
    } else if cfg!(target_os = "windows") {
        (
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-UICulture).Name",
            ],
        )
    } else {
        return None;
    };
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
        .find(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn explicit_from_args(args: &[String]) -> Option<String> {
    fn find(matches: &clap::ArgMatches) -> Option<String> {
        matches
            .get_one::<String>("lang")
            .cloned()
            .or_else(|| matches.subcommand().and_then(|(_, sub)| find(sub)))
    }
    // Reuse the authoritative grammar. Incomplete argv may still select a locale.
    let matches = crate::Cli::command()
        .disable_help_flag(true)
        .disable_version_flag(true)
        .ignore_errors(true)
        .try_get_matches_from(
            std::iter::once("spec-autonomous").chain(args.iter().map(String::as_str)),
        )
        .ok()?;
    find(&matches)
}

pub fn parse(args: &[String], locale: Locale) -> Result<crate::Cli, clap::Error> {
    let mut command = crate::Cli::command();
    command.build();
    let matches = native_help(localize_command(command, locale), locale).try_get_matches_from(
        std::iter::once("spec-autonomous").chain(args.iter().map(String::as_str)),
    )?;
    crate::Cli::from_arg_matches(&matches)
}

fn native_help(mut command: clap::Command, locale: Locale) -> clap::Command {
    if !locale.is_zh() {
        return command;
    }
    let ids: Vec<_> = command
        .get_arguments()
        .map(|arg| arg.get_id().to_string())
        .collect();
    for id in ids {
        command = command.mut_arg(&id, |arg| {
            let mut help = arg.get_help().map(ToString::to_string).unwrap_or_default();
            let defaults: Vec<_> = arg
                .get_default_values()
                .iter()
                .map(|v| v.to_string_lossy())
                .collect();
            if !defaults.is_empty() {
                help.push_str(&format!("（默认值：{}）", defaults.join(", ")));
            }
            let choices: Vec<_> = arg
                .get_possible_values()
                .into_iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| v.get_name().to_owned())
                .collect();
            if !choices.is_empty() {
                help.push_str(&format!("（可选值：{}）", choices.join(", ")));
            }
            arg.help(help)
                .hide_default_value(true)
                .hide_possible_values(true)
        });
    }
    let names: Vec<_> = command
        .get_subcommands()
        .map(|c| c.get_name().to_owned())
        .collect();
    for name in names {
        command = command.mut_subcommand(name, |sub| native_help(sub, locale));
    }
    command
}

pub fn catalog(locale: Locale) -> &'static BTreeMap<String, String> {
    static EN: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    static ZH: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    let (cache, source) = if locale.is_zh() {
        (
            &ZH,
            include_str!("../../../packages/cli/locales/zh-CN.json"),
        )
    } else {
        (&EN, include_str!("../../../packages/cli/locales/en.json"))
    };
    cache.get_or_init(|| {
        serde_json::from_str(source).expect("embedded locale catalog must be valid JSON")
    })
}

pub fn text(locale: Locale, key: &str) -> String {
    let value = catalog(locale).get(key).cloned();
    value
        .or_else(|| catalog(Locale::En).get(key).cloned())
        .unwrap_or_else(|| key.to_string())
}

pub fn error(locale: Locale, message: &str) -> String {
    if !locale.is_zh() {
        return message.to_owned();
    }
    let (code, detail) = message
        .split_once(':')
        .map_or((message, ""), |(c, d)| (c, d.trim()));
    let key = format!("error.{code}");
    let translated = text(locale, &key);
    if translated == key {
        return message.to_owned();
    }
    if detail.starts_with('/') && !detail.contains('\n') {
        format!("{translated}: {detail}")
    } else {
        translated
    }
}

pub fn clap_error(locale: Locale, message: &str) -> String {
    if message.starts_with("Usage:")
        || message.starts_with("error: unexpected")
        || message.starts_with("error:")
    {
        if locale.is_zh() {
            format!(
                "{}。使用 --help 查看用法。",
                text(locale, "error.invalid_arguments")
            )
        } else {
            message.to_string()
        }
    } else {
        message.to_string()
    }
}

pub fn localize_command(mut command: clap::Command, locale: Locale) -> clap::Command {
    if locale.is_zh() {
        command = command
            .help_template("{about-with-newline}\n用法：{usage}\n\n{all-args}")
            .subcommand_help_heading("命令");
    }
    let command_name = command.get_name().replace('-', "_");
    let key = if command_name == "spec_autonomous" {
        "cli.about".to_string()
    } else {
        format!("command.{command_name}")
    };
    let about = text(locale, &key);
    if about != key {
        command = command.about(about);
    }
    let ids: Vec<String> = command
        .get_arguments()
        .map(|arg| arg.get_id().as_str().to_string())
        .collect();
    for id in ids {
        if locale.is_zh() {
            command = command.mut_arg(&id, |arg| {
                let heading = if arg.is_positional() {
                    "参数"
                } else {
                    "选项"
                };
                arg.help_heading(heading)
            });
        }
        let key = format!("arg.{}", id.replace('-', "_"));
        let value = text(locale, &key);
        if value != key {
            command = command.mut_arg(&id, |arg| arg.help(value));
        }
    }
    let names: Vec<String> = command
        .get_subcommands()
        .map(|sub| sub.get_name().to_string())
        .collect();
    for name in names {
        command = command.mut_subcommand(&name, |sub| localize_command(sub, locale));
    }
    command
}

pub fn human(value: &serde_json::Value, locale: Locale) -> String {
    if !locale.is_zh() {
        return crate::progress::human(value);
    }
    let value = if value
        .get("data")
        .is_some_and(|d| d.get("worktrees").is_some())
    {
        &value["data"]
    } else {
        value
    };
    let Some(rows) = value.get("worktrees").and_then(serde_json::Value::as_array) else {
        if value
            .pointer("/data/next_action")
            .and_then(serde_json::Value::as_str)
            == Some(text(Locale::En, "human.init_next").as_str())
        {
            let mut translated = value.clone();
            translated["data"]["next_action"] = text(locale, "human.init_next").into();
            return serde_json::to_string_pretty(&translated).unwrap_or_default();
        }
        return serde_json::to_string_pretty(value).unwrap_or_default();
    };
    let label = |value: &str| {
        let key = format!("status.{value}");
        let translated = text(locale, &key);
        if translated == key {
            value.to_owned()
        } else {
            translated
        }
    };
    let mut out = format!(
        "Worktree：{} | 活跃 Worker：{} | 已验证任务：{} | 一致性：{}\n",
        rows.len(),
        value["active_workers"],
        value["verified_tasks"],
        label(value["consistency"].as_str().unwrap_or("unknown"))
    );
    for row in rows {
        out.push_str(&format!(
            "{}  {}  {}  {}\n",
            label(row["kind"].as_str().unwrap_or("")),
            label(row["status"].as_str().unwrap_or("")),
            row["phase_id"].as_str().unwrap_or(""),
            row["path"].as_str().unwrap_or("")
        ));
    }
    if let Some(ds) = value
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
    {
        for d in ds {
            out.push_str(&format!(
                "诊断：{}\n",
                error(locale, d.as_str().unwrap_or("unknown"))
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn human_translation_preserves_user_paths_and_unrelated_json() {
        let row = serde_json::json!({"kind":"external","status":"completed","phase_id":"unknown-external", "path":"/tmp/completed/unknown"});
        let value = serde_json::json!({"worktrees":[row],"active_workers":0,"verified_tasks":1,"consistency":"unknown"});
        let output = human(&value, Locale::ZhCn);
        assert!(output.contains("外部  已完成  unknown-external  /tmp/completed/unknown"));
        let arbitrary = serde_json::json!({"data":{"goal":"unknown completed external"}});
        assert_eq!(
            human(&arbitrary, Locale::ZhCn),
            serde_json::to_string_pretty(&arbitrary).unwrap()
        );
    }
    #[test]
    fn metadata_payload_help_is_parsed_without_intercepting_outer_command() {
        let args = ["--lang", "zh-CN", "cli-metadata", "parse", "--", "--help"].map(str::to_owned);
        assert!(parse(&args, Locale::ZhCn).is_ok());
        let args = ["cli-metadata", "parse", "--", "--lang", "zh-CN"].map(str::to_owned);
        assert_eq!(explicit_from_args(&args), None);
    }
    #[test]
    fn explicit_language_and_environment_precedence_are_stable() {
        assert_eq!(detect(Some("en-US")), Locale::En);
        assert_eq!(detect(Some("zh_CN.UTF-8")), Locale::ZhCn);
        assert_eq!(detect(Some("fr-FR")), Locale::En);
        assert!(zh("zh-Hans-CN"));
        assert!(!zh("en_US.UTF-8"));
    }
    #[test]
    fn catalogs_have_stable_keys_and_localize_help_and_errors() {
        let en = catalog(Locale::En);
        let zh = catalog(Locale::ZhCn);
        assert_eq!(en.keys().collect::<Vec<_>>(), zh.keys().collect::<Vec<_>>());
        assert!(text(Locale::ZhCn, "command.prepare").contains("准备"));
        assert_eq!(
            error(Locale::ZhCn, "invalid_path: /tmp/project"),
            "仓库路径无效: /tmp/project"
        );
        assert_eq!(
            error(Locale::ZhCn, "unknown_code: detail"),
            "unknown_code: detail"
        );
        let progress = serde_json::json!({"worktrees":[],"active_workers":0,"verified_tasks":0,"consistency":"unknown"});
        assert!(human(&progress, Locale::ZhCn).contains("活跃 Worker"));
    }
}
