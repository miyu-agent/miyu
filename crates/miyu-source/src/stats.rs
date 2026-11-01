//! 上线时刻和下载次数（施工 DL-2，设计 `54-下载量和排序.md` 第一、二节）：`miyu-index` 生成索引时多的几步。
//!
//! - [`carry_published`]：照上一份索引沿用 `published`：编号、版本对得上的照抄，新包、升了版的、上一份没有这一格的写这一次
//!   生成的时刻。
//! - [`downloads`]：问 GitHub 来源仓库的全部发布，把这个包的发布文件（`<编号>-<版本>[-<系统>].miyu`）的 `download_count`
//!   加起来：旧版不在索引里，照发布页上的全部算。
//! - [`Stats`]：软件源旁边的 `stats.json`，存最近 8 天每天的下载次数（照 UTC 的日期）；近 7 天的照那天的减 7 天前那一天的。
//!   不签名：只用来算趋势，交给核心的是索引里的数。
//! - [`count`]：带 `--github` 的那一步，把上面两样连起来写进索引。

use std::collections::BTreeMap;

use jiff::Span;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use crate::archive::EXTENSION;
use crate::collect::Fetch;
use crate::index::Index;

/// `stats.json` 认得的版本。
pub const VERSION: u32 = 1;

/// 留几天。
const KEPT_DAYS: i64 = 8;

/// 趋势照几天算。
const WEEK: i64 = 7;

/// GitHub 列发布一页最多几个。
const PER_PAGE: usize = 100;

/// 最多翻几页（一千个发布）。
const MOST_PAGES: usize = 10;

/// 一页发布的说明最多多大。
const MOST_PAGE: u64 = 32 << 20;

/// `stats.json`：最近几天每天的下载次数。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Stats {
    /// 写法的版本，现在是 [`VERSION`]。
    pub version: u32,
    /// 照日期排，最多 8 天。
    #[serde(default)]
    pub days: Vec<Day>,
}

/// 一天的：日期（`2026-10-31`，UTC）、包编号到那天为止一共被下载了几次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Day {
    /// 日期。
    pub date: String,
    /// 包编号到下载次数。
    #[serde(default)]
    pub downloads: BTreeMap<String, u64>,
}

impl Stats {
    /// 读 `stats.json`：读不成的、版本不认得的没有（从头攒）。
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Stats> {
        serde_json::from_slice::<Stats>(bytes)
            .ok()
            .filter(|stats| stats.version == VERSION)
    }

    /// 记下 `date` 这一天的：同一天的换掉，8 天以前的、比这天还晚的、日期写错的去掉，照日期排。
    pub fn record(&mut self, date: Date, downloads: BTreeMap<String, u64>) {
        self.version = VERSION;
        let oldest = date.saturating_sub(Span::new().days(KEPT_DAYS - 1));
        self.days.retain(|day| {
            day.date
                .parse::<Date>()
                .is_ok_and(|kept| oldest <= kept && kept < date)
        });
        self.days.push(Day {
            date: date.to_string(),
            downloads,
        });
        self.days.sort_by(|a, b| a.date.cmp(&b.date));
    }

    /// 包 `id` 近 7 天下载了几次：`date` 那天的减 7 天前那天的。哪一天没记、那天没有这个包的没有；少了的（删了发布）是 0。
    #[must_use]
    pub fn week(&self, date: Date, id: &str) -> Option<u64> {
        let on = |wanted: Date| {
            self.days
                .iter()
                .find(|day| day.date.parse::<Date>().ok() == Some(wanted))?
                .downloads
                .get(id)
                .copied()
        };
        let before = date.checked_sub(Span::new().days(WEEK)).ok()?;
        Some(on(date)?.saturating_sub(on(before)?))
    }
}

/// 照上一份索引 `previous` 填 `index` 每一条的 `published`：编号、版本对得上、上一份有这一格的照抄，别的写 `index` 生成的时刻。
pub fn carry_published(index: &mut Index, previous: Option<&Index>) {
    let generated = index.generated.clone();
    for entry in &mut index.packages {
        let kept = previous
            .and_then(|previous| {
                previous
                    .packages
                    .iter()
                    .find(|old| old.id == entry.id && old.version == entry.version)
            })
            .and_then(|old| old.published.clone());
        entry.published = Some(kept.unwrap_or_else(|| generated.clone()));
    }
}

/// GitHub 列发布回的里面用得上的。
#[derive(Debug, Deserialize)]
struct Release {
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    #[serde(default)]
    download_count: u64,
}

/// 来源仓库 `origin`（`github.com/<主>/<仓库>`）的全部发布里，包 `id` 的发布文件一共被下载了几次。`api` 是 GitHub API 的地址。
///
/// # Errors
///
/// 来源不在 GitHub 上、拿不到、读不懂：英文一句。
pub fn downloads(fetch: &dyn Fetch, api: &str, origin: &str, id: &str) -> Result<u64, String> {
    let repo = origin
        .strip_prefix("github.com/")
        .filter(|repo| repo.split('/').count() == 2)
        .ok_or_else(|| format!("{origin} is not on GitHub"))?;
    let mut total = 0;
    for page in 1..=MOST_PAGES {
        let url = format!(
            "{}/repos/{repo}/releases?per_page={PER_PAGE}&page={page}",
            api.trim_end_matches('/')
        );
        let releases: Vec<Release> = serde_json::from_slice(&fetch.get(&url, MOST_PAGE)?)
            .map_err(|error| format!("unreadable releases: {error}"))?;
        total += releases
            .iter()
            .flat_map(|release| &release.assets)
            .filter(|asset| file_of(&asset.name, id))
            .map(|asset| asset.download_count)
            .sum::<u64>();
        if releases.len() < PER_PAGE {
            break;
        }
    }
    Ok(total)
}

/// `name` 是不是包 `id` 的发布文件：`<编号>-<版本>[-<系统>].miyu`，版本照数字开头认（`memes-extra-1.0.miyu` 不算 `memes` 的）。
fn file_of(name: &str, id: &str) -> bool {
    name.strip_prefix(id)
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(|rest| rest.strip_suffix(&format!(".{EXTENSION}")))
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// 带 `--github` 的那一步：问 `index` 里每个包的下载次数，记进 `stats` 的 `date` 那天，写进每一条的 `downloads`、
/// `downloads_week`。问不成的（限流、网络错、仓库没了）那个包照上一份索引 `previous` 里同编号的两格沿用、不记进 `stats`，上一份
/// 也没有的不写；一个都问不成也不挡生成。交回一行一件事。
pub fn count(
    index: &mut Index,
    previous: Option<&Index>,
    stats: &mut Stats,
    date: Date,
    (fetch, api): (&dyn Fetch, &str),
) -> Vec<String> {
    let mut notes = Vec::new();
    let mut today = BTreeMap::new();
    let mut kept = Vec::new();
    for entry in &mut index.packages {
        match downloads(fetch, api, &entry.origin, &entry.id) {
            Ok(downloads) => {
                today.insert(entry.id.clone(), downloads);
                entry.downloads = Some(downloads);
            }
            Err(why) => match previous.and_then(|previous| previous.entry(&entry.id)) {
                Some(old) if old.downloads.is_some() => {
                    entry.downloads = old.downloads;
                    entry.downloads_week = old.downloads_week;
                    kept.push(entry.id.clone());
                    notes.push(format!(
                        "{}: downloads kept from the last index, {why}",
                        entry.id
                    ));
                }
                _ => notes.push(format!("{}: downloads left out, {why}", entry.id)),
            },
        }
    }
    stats.record(date, today);
    for entry in &mut index.packages {
        if entry.downloads.is_some() && !kept.contains(&entry.id) {
            entry.downloads_week = stats.week(date, &entry.id);
        }
    }
    notes
}

#[cfg(test)]
mod tests;
