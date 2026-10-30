//! `miyu ask` 等子代理、后台命令时给人看的字（施工 7-9，样子 2026-09-30 项目主人定；施工 AP-4 多了后台命令，
//! `docs/blueprint/cli/ask.md`「给人看的字」）。

use super::Language;

impl Language {
    /// 等的那一行：还有 `agents` 个子代理、`commands` 个后台命令没报，没有的那一样不写。
    pub(crate) fn waiting(&self, agents: usize, commands: usize) -> String {
        let what = self.counted(agents, commands);
        match self {
            Language::Chinese => format!("· 等待 {what}回报…（按 Ctrl+C 停止等待）"),
            Language::English => format!("· Waiting for {what} to report… (Ctrl+C stops waiting)"),
        }
    }

    /// 几个子代理、几个后台命令：`1 个子代理、2 个后台命令`、`1 subagent and 2 background commands`；是 0 的那一样不写，
    /// 两样都是 0 的照子代理说（等的是回报叫醒的那一轮，这一行本来不印）。
    fn counted(&self, agents: usize, commands: usize) -> String {
        let agents = (agents > 0 || commands == 0).then(|| match (self, agents) {
            (Language::Chinese, _) => format!("{agents} 个子代理"),
            (Language::English, 1) => "1 subagent".to_string(),
            (Language::English, _) => format!("{agents} subagents"),
        });
        let commands = (commands > 0).then(|| match (self, commands) {
            (Language::Chinese, _) => format!("{commands} 个后台命令"),
            (Language::English, 1) => "1 background command".to_string(),
            (Language::English, _) => format!("{commands} background commands"),
        });
        let joint = match self {
            Language::Chinese => "、",
            Language::English => " and ",
        };
        [agents, commands]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(joint)
    }

    /// 子代理、后台命令 `job` 报回来了，标题是 `title`：这一次以前派、这一次留了言的子代理，没见过派它的那一条，没有标题
    /// （施工 7-9 补）。
    pub(crate) fn reported(&self, job: &str, title: Option<&str>) -> String {
        match (self, title) {
            (Language::Chinese, Some(title)) => format!("· {job}「{title}」已回报"),
            (Language::Chinese, None) => format!("· {job} 已回报"),
            (Language::English, Some(title)) => {
                format!("· {job} \u{201c}{title}\u{201d} reported back")
            }
            (Language::English, None) => format!("· {job} reported back"),
        }
    }

    /// 还在跑的是哪几样：只有后台命令的、两样都有的照实说，别的照子代理说（施工 AP-4）。中文带上「仍在…运行」：只有后台
    /// 命令的不再说「后台」。
    fn still_running(&self, agents: usize, commands: usize) -> &'static str {
        match (self, agents > 0, commands > 0) {
            (Language::Chinese, false, true) => "后台命令仍在运行",
            (Language::Chinese, true, true) => "子代理和后台命令仍在后台运行",
            (Language::Chinese, _, false) => "子代理仍在后台运行",
            (Language::English, false, true) => "background commands",
            (Language::English, true, true) => "subagents and background commands",
            (Language::English, _, false) => "subagents",
        }
    }

    /// 等的时候按了 Ctrl+C：还有 `agents` 个子代理、`commands` 个后台命令没报。
    pub(crate) fn stopped_waiting(&self, agents: usize, commands: usize) -> String {
        let running = self.still_running(agents, commands);
        match self {
            Language::Chinese => format!("· 已停止等待，{running}，下次 miyu ask -c 时会收到结果"),
            Language::English => format!(
                "· Stopped waiting; the {running} keep running, and their results arrive at the next miyu ask -c"
            ),
        }
    }

    /// 到了 `--timeout`：还有 `agents` 个子代理、`commands` 个后台命令没报，都是 0 的只说到时间了。
    pub(crate) fn timed_out(&self, agents: usize, commands: usize) -> String {
        if agents == 0 && commands == 0 {
            return match self {
                Language::Chinese => "· 已到等待时限".to_string(),
                Language::English => "· Time is up".to_string(),
            };
        }
        let running = self.still_running(agents, commands);
        match self {
            Language::Chinese => format!("· 已到等待时限，未回报的{running}"),
            Language::English => {
                format!("· Time is up; the {running} that have not reported keep running")
            }
        }
    }
}
