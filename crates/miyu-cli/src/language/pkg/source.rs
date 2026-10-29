//! `miyu pkg update`、`search`、`clean`、`upgrade` 给人看的字（施工 G-3 起；CL-2 补从 `pkg.rs` 挪出来）。

use crate::diagnostic::Problem;
use crate::language::Language;

/// `miyu pkg update` 的字（施工 G-3 上；CL-2 补起一个软件源一行）。
impl Language {
    /// 软件源 `source` 拿到了新的索引：几个包、索引的时刻。
    pub(crate) fn index_updated(&self, source: &str, count: u64, generated: &str) -> String {
        match (self, count) {
            (Language::Chinese, _) => format!("已更新 {source}：{count} 个包（{generated}）"),
            (Language::English, 1) => format!("updated {source}: 1 package ({generated})"),
            (Language::English, _) => {
                format!("updated {source}: {count} packages ({generated})")
            }
        }
    }

    /// 软件源 `source` 没拿成：为什么。
    pub(crate) fn index_failed(&self, source: &str, why: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无法更新 {source}：{why}")),
            Language::English => Problem::new(format!("cannot update {source}: {why}")),
        }
    }

    /// 另加的软件源拿不成、本机也没有时核心给的原因码说成什么；不认得的照原样。
    pub(crate) fn source_error<'a>(&self, code: &'a str) -> &'a str {
        match (self, code) {
            (Language::Chinese, "source_unreachable") => "无法连接",
            (Language::English, "source_unreachable") => "unreachable",
            _ => code,
        }
    }

    /// 搜的时候软件源没连上，用的是本机那份（`generated` 是它的时刻）。
    pub(crate) fn index_stale(&self, generated: &str, why: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!(
                "无法连接软件源，使用本机索引（{generated}）：{why}"
            )),
            Language::English => Problem::new(format!(
                "cannot reach the package source, using the local index ({generated}): {why}"
            )),
        }
    }
}

/// `miyu pkg search` 的几个标记（施工 G-3 下）。
impl Language {
    /// `-S` 什么都没带（项目主人 2026-10-19 定照 pacman 说「没有指定目标」，CL-2 补照设计 47 改写法）。
    pub(crate) fn nothing_to_sync(&self) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new("没有指定目标").hint("运行 'miyu pkg --help' 查看用法")
            }
            Language::English => {
                Problem::new("no targets specified").hint("run 'miyu pkg --help' for usage")
            }
        }
    }

    /// `clean`：清掉了几个包文件、腾出多少；一个都没有的说没有要清的。
    pub(crate) fn cleaned(&self, removed: u64, freed: &str) -> String {
        match (self, removed) {
            (Language::Chinese, 0) => "没有可清理的包文件".to_string(),
            (Language::Chinese, _) => format!("已清理 {removed} 个包文件，释放 {freed}"),
            (Language::English, 0) => "no package files to clean".to_string(),
            (Language::English, 1) => format!("removed 1 package file, freed {freed}"),
            (Language::English, _) => format!("removed {removed} package files, freed {freed}"),
        }
    }

    /// 装着的。
    pub(crate) fn installed_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（已安装）",
            Language::English => " (installed)",
        }
    }

    /// 不是官方的：没人审过。
    pub(crate) fn community_mark(&self) -> &'static str {
        match self {
            Language::Chinese => "（社区）",
            Language::English => " (community)",
        }
    }

    /// 被拉黑了。
    pub(crate) fn blocked_mark(&self, reason: &str) -> String {
        match self {
            Language::Chinese => format!("（已拉黑：{reason}）"),
            Language::English => format!(" (blocked: {reason})"),
        }
    }
}

/// `miyu pkg upgrade` 的字（施工 G-5）。盘点里要说一句的四种是警告（照 pacman 的 `warning: … ignoring package upgrade`）。
impl Language {
    /// 没有要升级的。
    pub(crate) fn nothing_to_upgrade(&self) -> &'static str {
        match self {
            Language::Chinese => "没有可升级的软件包",
            Language::English => "nothing to upgrade",
        }
    }

    /// 升级好了一个：从 `from` 到 `to`。
    pub(crate) fn upgraded(&self, id: &str, from: &str, to: &str) -> String {
        match self {
            Language::Chinese => format!("已升级 {id} {from} → {to}"),
            Language::English => format!("upgraded {id} {from} → {to}"),
        }
    }

    /// 软件源里没有了：照旧用着。
    pub(crate) fn upgrade_gone(&self, id: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("{id} 已不在软件源中，保留已安装的版本")),
            Language::English => Problem::new(format!(
                "{id} is no longer in the package source, keeping the installed version"
            )),
        }
    }

    /// 被拉黑了：照旧开着，卸不卸自己定（建议怎么卸）。
    pub(crate) fn upgrade_blocked(&self, id: &str, reason: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("{id} 已被拉黑：{reason}"))
                .hint(format!("用 'miyu pkg remove {id}' 卸载")),
            Language::English => Problem::new(format!("{id} is blocked: {reason}"))
                .hint(format!("remove it with 'miyu pkg remove {id}'")),
        }
    }

    /// 来源仓库变了：不升。
    pub(crate) fn upgrade_moved(&self, id: &str, now: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("{id} 改由 {now} 提供，未升级")),
            Language::English => Problem::new(format!("{id} now comes from {now}, not upgraded")),
        }
    }

    /// `packages.hold` 里的：跳过。
    pub(crate) fn upgrade_held(&self, id: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("跳过设为不升级的 {id}")),
            Language::English => Problem::new(format!("skipping held package {id}")),
        }
    }
}
