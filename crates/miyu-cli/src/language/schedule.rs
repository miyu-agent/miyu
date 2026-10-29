//! `miyu schedule` 给人看的字（施工 J-3 下，`docs/blueprint/cli/schedule.md`「给人看的字」；CL-2 补照设计 47 第三节改：结果行
//! 动词打头、不带句号，报错交 [`Problem`]，建议另起一行）。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// `list` 一个都没有。
    pub fn no_schedules(&self) -> &'static str {
        match self {
            Language::Chinese => "没有定时任务",
            Language::English => "no schedules",
        }
    }

    /// `add` 建好了，接上那一项。
    pub(crate) fn schedule_added(&self, line: &str) -> String {
        match self {
            Language::Chinese => format!("已添加 {line}"),
            Language::English => format!("added {line}"),
        }
    }

    /// `rm` 删掉了。
    pub(crate) fn schedule_removed(&self, id: &str) -> String {
        match self {
            Language::Chinese => format!("已删除 {id}"),
            Language::English => format!("removed {id}"),
        }
    }

    /// `pause` 停了，接上那一项。
    pub(crate) fn schedule_paused(&self, line: &str) -> String {
        match self {
            Language::Chinese => format!("已暂停 {line}"),
            Language::English => format!("paused {line}"),
        }
    }

    /// `resume` 续上了，接上那一项。
    pub(crate) fn schedule_resumed(&self, line: &str) -> String {
        match self {
            Language::Chinese => format!("已恢复 {line}"),
            Language::English => format!("resumed {line}"),
        }
    }

    /// `--at` 读不懂。
    pub(crate) fn schedule_bad_at(&self, text: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无法识别时间 '{text}'"))
                .hint("写成 '2026-10-28 08:00'（本机时区）或 2026-10-28T08:00:00+09:00"),
            Language::English => Problem::new(format!("invalid time '{text}'"))
                .hint("use '2026-10-28 08:00' (local time) or 2026-10-28T08:00:00+09:00"),
        }
    }

    /// `--every` 读不懂。
    pub(crate) fn schedule_bad_every(&self, text: &str) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new(format!("无法识别间隔 '{text}'")).hint("写秒数，或 90s、10m、2h、1d")
            }
            Language::English => Problem::new(format!("invalid interval '{text}'"))
                .hint("use seconds, or 90s, 10m, 2h, 1d"),
        }
    }

    /// 核心拒了 `schedule.add`、`schedule.resume` 时 `data.problem` 说的是哪一样；不认得的没有。
    pub(crate) fn schedule_problem(&self, problem: &str) -> Option<&'static str> {
        let (chinese, english) = match problem {
            "when" => ("时间格式不对", "invalid time"),
            "cron" => (
                "cron 表达式无效，需要标准五段",
                "invalid cron expression, use the standard five fields",
            ),
            "past" => ("时间已经过去", "time already passed"),
            "short" => ("间隔至少 60 秒", "interval must be at least 60 seconds"),
            "prompt" => ("消息为空", "empty message"),
            "session" => ("会话不存在", "no such session"),
            "target" => (
                "会话和新建会话的选项只能用一种",
                "use either a session or new-session options, not both",
            ),
            "zone" => ("未知时区", "unknown time zone"),
            "cwd" => ("目录不存在或不可用", "directory missing or unusable"),
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
            (Language::Chinese, "dynamic") => "循环，节奏自定",
            (Language::Chinese, _) => "循环",
            (Language::English, "dynamic") => "loop, self-paced",
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
            (Language::Chinese, "ran") => "已运行",
            (Language::Chinese, "queued") => "已排队",
            (Language::Chinese, "sent") => "已发给软件包",
            (Language::English, "sent") => "sent to package",
            (Language::Chinese, "skipped:package_unavailable") => "跳过：软件包未运行",
            (Language::English, "skipped:package_unavailable") => "skipped: package not running",
            (Language::Chinese, "skipped:session_gone") => "跳过：会话已不存在",
            (Language::Chinese, "skipped:preset_off") => "跳过：预设已关闭",
            (Language::Chinese, "skipped:not_opened") => "跳过：无法新建会话",
            (Language::English, "skipped:session_gone") => "skipped: session gone",
            (Language::English, "skipped:preset_off") => "skipped: disabled by preset",
            (Language::English, "skipped:not_opened") => "skipped: new session failed to open",
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
            Language::Chinese => "只运行一次",
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
            Language::Chinese => format!("发给软件包 {package}"),
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
            Language::Chinese => ("新会话", "人格", "无人格", "预设", "目录"),
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
