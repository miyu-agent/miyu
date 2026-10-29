//! `miyu pkg` 里软件源的几个（施工 G-3 上起，设计 `37-软件源.md` 第四节）：`update`（pacman 的 `-Sy`）拿新的索引，`search`
//! （`-Ss`，施工 G-3 下）在软件源里搜；`-Si`（施工 IN-2）换成 `info`。之后的从软件源装、`upgrade` 也住这里（`pkg.rs` 到了 500 行）。

use std::io::Write;

use clap::Args;
use serde_json::{Value, json};

use super::{PkgCommand, ask};
use crate::diagnostic;
use crate::exit;
use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::say;

/// `-S` 后面接的（施工 G-3 上、下，G-4 下）：`-y` 拿索引，`-s` 搜，`-c` 清缓存，接几个词；不带 `-s`、`-c` 只带词的是从软件源
/// 装这几个（`-Sy <编号>` 先拿再装）。字母、词都没写的照 pacman 说「没有指定目标」，退出码 2（项目主人 2026-10-19 定）。
#[derive(Debug, Clone, Args)]
pub struct SyncFlags {
    /// `-Sy`：`update`。
    #[arg(short = 'y')]
    refresh: bool,
    /// `-Ss`：`search`。
    #[arg(short = 's')]
    search: bool,
    /// `-Sc`：`clean`（施工 G-4 下）。
    #[arg(short = 'c')]
    clean: bool,
    /// `-Su`：`upgrade`（施工 G-5），`-Syu` 先拿再升。
    #[arg(short = 'u')]
    upgrade: bool,
    /// `-Si <编号>`：只查软件源的 `info`（施工 IN-2），接一个编号，不和别的字母、词一起写。
    #[arg(short = 'i', value_name = "ID", conflicts_with_all = ["refresh", "search", "clean", "upgrade", "words"])]
    info: Option<String>,
    /// `-Ss` 搜的词。
    words: Vec<String>,
}

impl SyncFlags {
    /// `-Si <编号>` 换成只查软件源的 `info`（施工 IN-2），别的照旧。
    pub(super) fn plain(self) -> PkgCommand {
        match self.info {
            Some(package) => PkgCommand::Info {
                package,
                source: Some(true),
            },
            None => PkgCommand::Sync(self),
        }
    }

    /// 从软件源装的那几个：不带 `-s`、`-c`，写了编号的。
    pub(super) fn installs(&self) -> Option<&[String]> {
        (!self.search && !self.clean && !self.upgrade && !self.words.is_empty())
            .then_some(self.words.as_slice())
    }

    /// 带了 `-u`。
    pub(super) fn upgrade(&self) -> bool {
        self.upgrade
    }

    /// 带了 `-y`。
    pub(super) fn refresh(&self) -> bool {
        self.refresh
    }

    /// 带了 `-s` 的搜的词（照空格连起来）；没带的没有（会话里的 `miyu pkg`，施工 BS-1 中下）。
    pub(super) fn searched(&self) -> Option<String> {
        self.search.then(|| self.words.join(" "))
    }

    /// 带了 `-c`。
    pub(super) fn cleans(&self) -> bool {
        self.clean
    }
}

/// pacman 的 `-S…`：照几个字母做。
pub(super) async fn pacman(
    rpc: &mut Rpc,
    flags: SyncFlags,
    language: &Language,
    json: bool,
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    if flags.clean {
        return clean(rpc, language, json, (out, err)).await;
    }
    if !flags.refresh && !flags.search {
        diagnostic::error(err, *language, &language.nothing_to_sync());
        return Err(exit::MISUSE);
    }
    if flags.refresh {
        update(rpc, language, json, (out, err)).await?;
    }
    if flags.search {
        search(rpc, &flags.words.join(" "), language, json, (out, err)).await?;
    }
    Ok(())
}

/// `search`：一行一个，`<编号>  <名字>  <说明>`，编号照最长的对齐；装着的接「已装」，社区的接「社区」，拉黑的接理由。一个都
/// 没搜到的什么都不印、退出码 1（照 pacman）；用的是本机的旧索引的在标准错误上说一句。`json` 的原样印回应。
pub(super) async fn search(
    rpc: &mut Rpc,
    query: &str,
    language: &Language,
    json: bool,
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let found = ask(
        rpc,
        "package.search",
        json!({"query": query}),
        language,
        err,
    )
    .await?;
    if let Some(why) = found["stale"].as_str() {
        let generated = found["generated"].as_str().unwrap_or_default();
        diagnostic::warning(err, *language, &language.index_stale(generated, why));
    }
    let rows = found["packages"].as_array().cloned().unwrap_or_default();
    if json {
        say(out, &found.to_string());
    } else {
        for line in shown(&rows, language) {
            say(out, &line);
        }
    }
    if rows.is_empty() {
        Err(exit::ERROR)
    } else {
        Ok(())
    }
}

/// 搜到的一行一个。
fn shown(rows: &[Value], language: &Language) -> Vec<String> {
    let text = |row: &Value, key: &str| row[key].as_str().unwrap_or_default().to_string();
    let width = rows
        .iter()
        .map(|row| text(row, "package").chars().count())
        .max()
        .unwrap_or(0);
    rows.iter()
        .map(|row| {
            let line = format!(
                "{:width$}  {}  {}",
                text(row, "package"),
                text(row, "name"),
                text(row, "summary")
            );
            let mut line = line.trim_end().to_string();
            if row["installed"].is_string() {
                line.push_str(language.installed_mark());
            }
            if row["official"].as_bool() != Some(true) {
                line.push_str(language.community_mark());
            }
            if let Some(reason) = row["blocked"].as_str() {
                line.push_str(&language.blocked_mark(reason));
            }
            line
        })
        .collect()
}

/// `clean`（`-Sc`，施工 G-4 下）：清掉缓存里不是装着那一版的包文件，说清了几个、腾出多少。`json` 的原样印回应。
pub(super) async fn clean(
    rpc: &mut Rpc,
    language: &Language,
    json: bool,
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let cleaned = ask(rpc, "package.clean", json!({}), language, err).await?;
    if json {
        say(out, &cleaned.to_string());
        return Ok(());
    }
    let removed = cleaned["removed"].as_u64().unwrap_or_default();
    let freed = super::query::size(cleaned["freed"].as_u64().unwrap_or_default());
    say(out, &language.cleaned(removed, &freed));
    Ok(())
}

/// 官方的软件源在回应里叫什么（配置里起名字的那几个在 `sources` 里各带 `source`）。
const OFFICIAL: &str = "official";

/// `update`：不管本机那份新不新都去拿（`package.sync` 带 `force`）。一个软件源一行（施工 CL-2 补，官方的在前，配置里另加的
/// 照回应的先后）：拿成了印包数和索引的时刻；官方的一处都没拿成、用的是本机那份的报错、退出码 1（照 pacman：`-Sy` 没拿成算
/// 失败）；另加的没拿成的警告一句，不算失败。`json` 的原样印回应，报错、警告照样印在标准错误上。
pub(super) async fn update(
    rpc: &mut Rpc,
    language: &Language,
    json: bool,
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let synced = ask(rpc, "package.sync", json!({"force": true}), language, err).await?;
    if json {
        say(out, &synced.to_string());
    }
    let official = (OFFICIAL, &synced);
    let others = synced["sources"].as_array().into_iter().flatten();
    let mut failed = false;
    for (index, (name, one)) in std::iter::once(official)
        .chain(others.map(|one| (one["source"].as_str().unwrap_or_default(), one)))
        .enumerate()
    {
        // 用的是本机那份的带 `stale`（最后一处的原因），本机也没有的是 `error`（原因码）。
        let why = match (one["stale"].as_str(), one["error"].as_str()) {
            (Some(why), _) => Some(why),
            (None, Some(code)) => Some(language.source_error(code)),
            (None, None) => None,
        };
        match (why, index) {
            (Some(why), 0) => {
                diagnostic::error(err, *language, &language.index_failed(name, why));
                failed = true;
            }
            (Some(why), _) => {
                diagnostic::warning(err, *language, &language.index_failed(name, why));
            }
            (None, _) if !json => {
                let count = one["packages"].as_u64().unwrap_or_default();
                let generated = one["generated"].as_str().unwrap_or_default();
                say(out, &language.index_updated(name, count, generated));
            }
            (None, _) => {}
        }
    }
    if failed { Err(exit::ERROR) } else { Ok(()) }
}
