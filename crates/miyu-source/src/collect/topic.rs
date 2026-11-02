//! 社区来源（施工 CC-1，设计 `37-软件源.md` 第三节第 5 条）：照 GitHub 的搜索列出打了某个话题的仓库，交给收录照官方那一套收。
//!
//! - 搜 `{api}/search/repositories?q=topic:<话题>&per_page=100&page=<n>`，一页不满就停；搜索最多给 1000 个，到 [`MOST_PAGES`]
//!   页就停、记一笔。
//! - 分叉的不收：抄一份官方仓库、打上话题的不该和原仓库抢。存档了的不收：作者不再维护。名字不是 `owner/repo` 的不拼进地址。
//!   各记一笔。
//! - 交回的照建仓库的时刻排（一样的照名字）：同一趟两个仓库头一回占同一个编号时，建得早的那个先收、先占（`names.toml` 先到先得）。
//! - 搜不成（HTTP 出错、JSON 读不成）的记一笔，已经搜到的照交，别的来源照收：社区这一头坏了不挡官方的发布。

use serde::Deserialize;

use super::rules;

/// 一页要多少个：GitHub 搜索的上限。
const PER_PAGE: usize = 100;

/// 最多翻几页：GitHub 搜索最多给 1000 个，`1000 / PER_PAGE`。
const MOST_PAGES: usize = 10;

/// 一页搜索结果最多多大。
const MOST_PAGE_BYTES: u64 = 16 << 20;

/// 一页搜索结果里用得上的。
#[derive(Debug, Deserialize)]
struct Page {
    #[serde(default)]
    incomplete_results: bool,
    items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    full_name: String,
    #[serde(default)]
    fork: bool,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    created_at: String,
}

/// 打了话题 `topic` 的仓库（`owner/repo`），照建仓库的时刻排；跳过的、搜不成的、到上限的记进 `notes`。
pub(super) fn search(
    topic: &str,
    fetch: &dyn super::Fetch,
    api: &str,
    notes: &mut Vec<String>,
) -> Vec<String> {
    let mut found: Vec<Item> = Vec::new();
    for page in 1..=MOST_PAGES {
        let url = format!(
            "{}/search/repositories?q=topic:{topic}&per_page={PER_PAGE}&page={page}",
            api.trim_end_matches('/')
        );
        let read = fetch.get(&url, MOST_PAGE_BYTES).and_then(|bytes| {
            serde_json::from_slice::<Page>(&bytes)
                .map_err(|error| format!("unreadable search results: {error}"))
        });
        let one = match read {
            Ok(one) => one,
            Err(why) => {
                notes.push(format!("topic {topic}: search failed, {why}"));
                break;
            }
        };
        if one.incomplete_results {
            notes.push(format!("topic {topic}: search results incomplete"));
        }
        let short = one.items.len() < PER_PAGE;
        found.extend(one.items);
        if short {
            break;
        }
        if page == MOST_PAGES {
            notes.push(format!(
                "topic {topic}: stopped at {MOST_PAGES} pages, the search returns at most {} repositories",
                MOST_PAGES * PER_PAGE
            ));
        }
    }
    notes.push(format!("topic {topic}: {} repositories", found.len()));
    found.sort_by(|a, b| {
        (a.created_at.as_str(), a.full_name.as_str())
            .cmp(&(b.created_at.as_str(), b.full_name.as_str()))
    });
    let mut repos = Vec::new();
    for item in found {
        let repo = item.full_name;
        if !rules::valid_repo(&repo) {
            notes.push(format!("topic {topic}: skipped {repo:?}, not owner/repo"));
        } else if item.fork {
            notes.push(format!("{repo}: skipped, a fork"));
        } else if item.archived {
            notes.push(format!("{repo}: skipped, archived"));
        } else {
            repos.push(repo);
        }
    }
    repos
}
