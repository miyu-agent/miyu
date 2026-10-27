//! `miyu schedule` 给人看的字（施工 J-3 下，`docs/blueprint/cli/schedule.md`「给人看的字」）。

use super::Language;

impl Language {
    /// `list` 一个都没有。
    pub fn no_schedules(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有定时任务。",
            Language::English => "No schedules yet.",
        }
    }

    /// `add` 建好了，接上那一项。
    pub(crate) fn schedule_added(&self, line: &str) -> String {
        match self {
            Language::Chinese => format!("建好了：{line}"),
            Language::English => format!("Added: {line}"),
        }
    }

    /// `rm` 删掉了。
    pub(crate) fn schedule_removed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("删掉了：{id}"),
            Language::English => format!("Removed: {id}"),
        }
    }

    /// `pause` 停了，接上那一项。
    pub(crate) fn schedule_paused(&self, line: &str) -> String {
        match self {
            Language::Chinese => format!("停了：{line}"),
            Language::English => format!("Paused: {line}"),
        }
    }

    /// `resume` 续上了，接上那一项。
    pub(crate) fn schedule_resumed(&self, line: &str) -> String {
        match self {
            Language::Chinese => format!("续上了：{line}"),
            Language::English => format!("Resumed: {line}"),
        }
    }

    /// `--at` 读不懂。
    pub(crate) fn schedule_bad_at(&self, text: &str) -> String {
        match self {
            Language::Chinese => format!(
                "读不懂 --at {text}：写成 2026-10-28 08:00（照本机时区），或者带偏移的 2026-10-28T08:00:00+09:00。"
            ),
            Language::English => format!(
                "Can't read --at {text}: write 2026-10-28 08:00 (this machine's time zone), or with an offset like 2026-10-28T08:00:00+09:00."
            ),
        }
    }

    /// `--every` 读不懂。
    pub(crate) fn schedule_bad_every(&self, text: &str) -> String {
        match self {
            Language::Chinese => {
                format!("读不懂 --every {text}：写秒数，或者 90s、10m、2h、1d。")
            }
            Language::English => {
                format!("Can't read --every {text}: write seconds, or 90s, 10m, 2h, 1d.")
            }
        }
    }

    /// 核心拒了 `schedule.add`、`schedule.resume` 时 `data.problem` 说的是哪一样；不认得的没有。
    pub(crate) fn schedule_problem(&self, problem: &str) -> Option<&'static str> {
        let (chinese, english) = match problem {
            "when" => ("时间没写对", "the time isn't written right"),
            "cron" => (
                "cron 读不懂，要标准五段",
                "the cron can't be read; use the standard five fields",
            ),
            "past" => ("那个时刻已经过了", "that time has already passed"),
            "short" => ("间隔至少 60 秒", "the interval must be at least 60 seconds"),
            "prompt" => ("话是空的", "the message is empty"),
            "session" => ("没有这个会话", "no such session"),
            "target" => (
                "会话和新开只能写一种",
                "give either a session or new-session options, not both",
            ),
            "zone" => ("不认识这个时区", "unknown time zone"),
            "cwd" => (
                "目录不在、不是目录，或者不能用",
                "the folder doesn't exist, isn't a folder, or can't be used",
            ),
            _ => return None,
        };
        Some(match self {
            Language::Chinese => chinese,
            Language::English => english,
        })
    }

    /// 循环的那一种。
    pub(crate) fn schedule_loop(&self, kind: &str) -> &'static str {
        match (self, kind) {
            (Language::Chinese, "dynamic") => "循环，她定节奏",
            (Language::Chinese, _) => "循环",
            (Language::English, "dynamic") => "loop, her pace",
            (Language::English, _) => "loop",
        }
    }

    /// 停着。
    pub(crate) fn schedule_paused_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "停着",
            Language::English => "paused",
        }
    }

    /// 下一次。
    pub(crate) fn schedule_next(&self, at: &str) -> String {
        match self {
            Language::Chinese => format!("下一次 {at}"),
            Language::English => format!("next {at}"),
        }
    }

    /// 上一次，接上结果。
    pub(crate) fn schedule_last(&self, at: &str, result: &str) -> String {
        match self {
            Language::Chinese => format!("上一次 {at} {result}"),
            Language::English => format!("last {at} {result}"),
        }
    }

    /// 上一次的结果：`ran`、`queued`、`sent`、`skipped:<原因>`；不认得的照原样。
    pub(crate) fn schedule_result(&self, result: &str) -> String {
        let said = match (self, result) {
            (Language::Chinese, "ran") => "跑了",
            (Language::Chinese, "queued") => "排在后面",
            (Language::Chinese, "sent") => "推给了包",
            (Language::English, "sent") => "sent to package",
            (Language::Chinese, "skipped:package_unavailable") => "跳过：包没在跑",
            (Language::English, "skipped:package_unavailable") => "skipped: package not running",
            (Language::Chinese, "skipped:session_gone") => "跳过：会话没了",
            (Language::Chinese, "skipped:preset_off") => "跳过：预设关了它",
            (Language::Chinese, "skipped:not_opened") => "跳过：新会话开不成",
            (Language::English, "skipped:session_gone") => "skipped: session gone",
            (Language::English, "skipped:preset_off") => "skipped: preset turned it off",
            (Language::English, "skipped:not_opened") => "skipped: new session didn't open",
            (_, other) => other,
        };
        said.to_string()
    }

    /// 那句话。
    pub(crate) fn schedule_prompt(&self, prompt: &str) -> String {
        match self {
            Language::Chinese => format!("「{prompt}」"),
            Language::English => format!("\"{prompt}\""),
        }
    }

    /// `cron` 的：写了时区、只跑一次的跟在括号里。
    pub(crate) fn schedule_cron(&self, cron: &str, zone: Option<&str>, once: bool) -> String {
        let once = once.then_some(match self {
            Language::Chinese => "只跑一次",
            Language::English => "once",
        });
        let notes: Vec<&str> = zone.into_iter().chain(once).collect();
        match (self, notes.is_empty()) {
            (_, true) => format!("cron {cron}"),
            (Language::Chinese, false) => format!("cron {cron}（{}）", notes.join("，")),
            (Language::English, false) => format!("cron {cron} ({})", notes.join(", ")),
        }
    }

    /// 跑在已有的会话上。
    pub(crate) fn schedule_on_session(&self, session: &str) -> String {
        match self {
            Language::Chinese => format!("会话 {session}"),
            Language::English => format!("session {session}"),
        }
    }

    /// 推给建它的包（施工 J-4）。
    pub(crate) fn schedule_on_package(&self, package: &str) -> String {
        match self {
            Language::Chinese => format!("推给包 {package}"),
            Language::English => format!("to package {package}"),
        }
    }

    /// 每次新开：人格（`Some(None)` 是无人格）、预设、目录写了的跟着写。
    pub(crate) fn schedule_on_new(
        &self,
        persona: Option<Option<&str>>,
        preset: Option<&str>,
        cwd: Option<&str>,
    ) -> String {
        let (new, as_persona, none, as_preset, in_dir) = match self {
            Language::Chinese => ("新开", "人格", "无人格", "预设", "目录"),
            Language::English => ("new", "persona", "no persona", "preset", "in"),
        };
        let mut said = vec![new.to_string()];
        match persona {
            Some(Some(persona)) => said.push(format!("{as_persona} {persona}")),
            Some(None) => said.push(none.to_string()),
            None => {}
        }
        if let Some(preset) = preset {
            said.push(format!("{as_preset} {preset}"));
        }
        if let Some(cwd) = cwd {
            said.push(format!("{in_dir} {cwd}"));
        }
        said.join(" ")
    }
}
