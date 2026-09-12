use std::collections::BTreeMap;

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
        .to_ascii_lowercase()
        .replace('_', "-");
    value == "zh" || value.starts_with("zh-") || value.starts_with("cmn")
}

pub fn detect(explicit: Option<&str>) -> Locale {
    if let Some(explicit) = explicit {
        return if zh(explicit) {
            Locale::ZhCn
        } else {
            Locale::En
        };
    }
    for key in [
        "SPEC_AUTONOMOUS_LANG",
        "LC_ALL",
        "LC_MESSAGES",
        "LANGUAGE",
        "LANG",
    ] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if value.eq_ignore_ascii_case("C") || value.eq_ignore_ascii_case("POSIX") {
                return Locale::En;
            }
            return if zh(value) { Locale::ZhCn } else { Locale::En };
        }
    }
    Locale::En
}

pub fn explicit_from_args(args: &[String]) -> Option<String> {
    let own = args.split(|arg| arg == "--").next().unwrap_or(args);
    own.windows(2)
        .find(|pair| pair[0] == "--lang")
        .map(|pair| pair[1].clone())
        .or_else(|| {
            own.iter()
                .find_map(|arg| arg.strip_prefix("--lang=").map(ToString::to_string))
        })
}

pub fn catalog(locale: Locale) -> BTreeMap<String, String> {
    let source = if locale.is_zh() {
        include_str!("../../../packages/cli/locales/zh-CN.json")
    } else {
        include_str!("../../../packages/cli/locales/en.json")
    };
    serde_json::from_str(source).expect("embedded locale catalog must be valid JSON")
}

pub fn text(locale: Locale, key: &str) -> String {
    let value = catalog(locale).get(key).cloned();
    value
        .or_else(|| catalog(Locale::En).get(key).cloned())
        .unwrap_or_else(|| key.to_string())
}

pub fn error(locale: Locale, message: &str) -> String {
    let (code, detail) = message
        .split_once(':')
        .map_or((message, ""), |(code, detail)| (code, detail.trim_start()));
    let key = if code == "error" || message.starts_with("Usage:") {
        "error.invalid_arguments".to_string()
    } else {
        format!("error.{code}")
    };
    let translated = text(locale, &key);
    if translated == key {
        return message.to_string();
    }
    if detail.is_empty() || code == "error" {
        translated
    } else {
        format!("{translated}: {detail}")
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
    let mut output = crate::progress::human(value);
    if locale.is_zh() {
        for (from, to) in [
            ("Worktrees", "Worktree"),
            ("active workers", "活跃 Worker"),
            ("verified tasks", "已验证任务"),
            ("consistency", "一致性"),
            ("unknown", "未知"),
            ("Diagnostic", "诊断"),
            ("managed-integration", "托管集成"),
            ("managed-worker", "托管 Worker"),
            ("external", "外部"),
            ("claimed", "已领取"),
            ("issued", "已发布"),
            ("submitted", "已提交"),
            ("completed", "已完成"),
            ("paused", "已暂停"),
            ("blocked", "已阻塞"),
            ("prunable", "可清理"),
        ] {
            output = output.replace(from, to);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
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
