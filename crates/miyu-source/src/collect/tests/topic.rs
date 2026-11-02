//! 社区自动收录（施工 CC-1，设计 `37-软件源.md` 第三节第 5 条）：搜索换成内存里的几页。收成社区的一条、分叉和存档的跳过、官方
//! 仓库也打了话题的不收两遍、编号被占的跳过、拉黑的去掉（拉黑仓库的连下都不下）、搜不成的照收官方的、短的一页就停、到 10 页就停、
//! 同一趟两个仓库头一回占同一个编号建得早的占；话题的写法不对的读规则时报。

use std::cell::RefCell;

use super::*;

/// 搜索第 `page` 页的地址。
fn page_url(page: u32) -> String {
    format!("{API}/search/repositories?q=topic:miyu-package&per_page=100&page={page}")
}

/// 一个搜到的仓库：名字、是不是分叉、是不是存档了、什么时候建的。
fn item(full_name: &str, fork: bool, archived: bool, created_at: &str) -> serde_json::Value {
    serde_json::json!({
        "full_name": full_name,
        "fork": fork,
        "archived": archived,
        "created_at": created_at,
    })
}

impl Net {
    /// 搜索第 `page` 页交回这几个仓库。
    fn search_page(&mut self, page: u32, items: &[serde_json::Value]) {
        self.0.insert(
            page_url(page),
            serde_json::to_vec(&serde_json::json!({
                "total_count": items.len(),
                "incomplete_results": false,
                "items": items,
            }))
            .expect("写得成"),
        );
    }
}

/// 记下问过哪些地址的网络。
struct Asked<'a> {
    net: &'a Net,
    urls: RefCell<Vec<String>>,
}

impl Fetch for Asked<'_> {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.urls.borrow_mut().push(url.to_string());
        self.net.get(url, limit)
    }
}

/// 只有社区来源的规则；`official` 列的仓库也写进 `official.toml`。
fn topic_rules(official: &[&str]) -> Rules {
    Rules {
        sources: vec![Source::Topic("miyu-package".to_string())],
        official: official.iter().map(|repo| (*repo).to_string()).collect(),
        ..Rules::default()
    }
}

fn run(rules: &Rules, fetch: &dyn Fetch, scratch: &Scratch) -> Collected {
    collect(
        rules,
        fetch,
        API,
        &scratch_dir(scratch),
        "2026-11-02T03:00:00Z",
    )
}

fn said(got: &Collected) -> String {
    got.notes.join("\n")
}

#[test]
fn a_tagged_repo_is_collected_as_community() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let hello = package(scratch.path(), "hello", "0.1.0", "", None);
    net.release("someone/miyu-hello", &[hello]);
    net.search_page(
        1,
        &[item(
            "someone/miyu-hello",
            false,
            false,
            "2026-10-01T00:00:00Z",
        )],
    );
    let got = run(&topic_rules(&[]), &net, &scratch);
    assert_eq!(got.index.packages.len(), 1, "{}", said(&got));
    let entry = &got.index.packages[0];
    assert_eq!(entry.id, "hello");
    assert_eq!(entry.origin, "github.com/someone/miyu-hello");
    assert!(!entry.official, "不在 official.toml 的是社区的");
    assert_eq!(got.names["hello"], "github.com/someone/miyu-hello");
    assert!(
        said(&got).contains("topic miyu-package: 1 repositories"),
        "{}",
        said(&got)
    );
}

#[test]
fn forks_archived_and_bad_names_are_skipped() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    for repo in ["copier/miyu-memes", "old/miyu-old"] {
        let one = package(
            scratch.path(),
            repo.replace('/', "-").as_str(),
            "1.0",
            "",
            None,
        );
        net.release(repo, &[one]);
    }
    net.search_page(
        1,
        &[
            item("copier/miyu-memes", true, false, "2026-10-01T00:00:00Z"),
            item("old/miyu-old", false, true, "2026-10-01T00:00:00Z"),
            item("../x", false, false, "2026-10-01T00:00:00Z"),
        ],
    );
    let asked = Asked {
        net: &net,
        urls: RefCell::default(),
    };
    let got = run(&topic_rules(&[]), &asked, &scratch);
    assert!(got.index.packages.is_empty(), "{}", said(&got));
    for expected in [
        "copier/miyu-memes: skipped, a fork",
        "old/miyu-old: skipped, archived",
        "topic miyu-package: skipped \"../x\", not owner/repo",
    ] {
        assert!(
            said(&got).contains(expected),
            "少了「{expected}」：\n{}",
            said(&got)
        );
    }
    let urls = asked.urls.borrow();
    assert!(
        !urls.iter().any(|url| url.contains("/repos/")),
        "跳过的连 Release 都不问：{urls:?}"
    );
}

#[test]
fn an_official_repo_with_the_topic_is_collected_once_as_official() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let memes = package(scratch.path(), "memes", "1.0", "", None);
    net.release("miyu-agent/miyu-memes", &[memes]);
    net.search_page(
        1,
        &[item(
            "miyu-agent/miyu-memes",
            false,
            false,
            "2026-10-01T00:00:00Z",
        )],
    );
    for sources in [
        vec![Source::Official, Source::Topic("miyu-package".to_string())],
        vec![Source::Topic("miyu-package".to_string()), Source::Official],
    ] {
        let mut rules = topic_rules(&["miyu-agent/miyu-memes"]);
        rules.sources = sources;
        let got = run(&rules, &net, &scratch);
        assert_eq!(got.index.packages.len(), 1, "{}", said(&got));
        assert!(
            got.index.packages[0].official,
            "官方的那一格照 official.toml"
        );
        assert_eq!(
            got.notes
                .iter()
                .filter(|note| note.as_str() == "miyu-agent/miyu-memes: memes 1.0")
                .count(),
            1,
            "{}",
            said(&got)
        );
    }
}

#[test]
fn an_id_owned_by_another_repo_is_skipped_and_the_older_repo_wins_a_new_one() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let memes = package(scratch.path(), "memes", "1.0", "", None);
    net.release("squatter/memes", &[memes]);
    let (name, bytes) = package(scratch.path(), "fresh", "1.0", "", None);
    net.release("newer/fresh", &[(name.clone(), bytes.clone())]);
    net.release("older/fresh", &[(name, bytes)]);
    net.search_page(
        1,
        &[
            item("squatter/memes", false, false, "2026-01-01T00:00:00Z"),
            item("newer/fresh", false, false, "2026-10-02T00:00:00Z"),
            item("older/fresh", false, false, "2026-10-01T00:00:00Z"),
        ],
    );
    let mut rules = topic_rules(&[]);
    rules.names.insert(
        "memes".to_string(),
        "github.com/miyu-agent/miyu-memes".to_string(),
    );
    let got = run(&rules, &net, &scratch);
    for expected in [
        "squatter/memes: skipped, id memes belongs to github.com/miyu-agent/miyu-memes",
        "older/fresh: claimed id fresh",
        "newer/fresh: skipped, id fresh belongs to github.com/older/fresh",
    ] {
        assert!(
            said(&got).contains(expected),
            "少了「{expected}」：\n{}",
            said(&got)
        );
    }
    assert_eq!(got.index.packages.len(), 1, "{}", said(&got));
    assert_eq!(got.index.packages[0].origin, "github.com/older/fresh");
}

#[test]
fn blocked_repos_and_ids_are_left_out_and_a_blocked_repo_is_not_downloaded() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    let evil = package(scratch.path(), "evil", "1.0", "", None);
    net.release("bad/evil", &[evil]);
    let spam = package(scratch.path(), "spam", "1.0", "", None);
    net.release("spammer/spam", &[spam]);
    net.search_page(
        1,
        &[
            item("bad/evil", false, false, "2026-10-01T00:00:00Z"),
            item("spammer/spam", false, false, "2026-10-01T00:00:00Z"),
        ],
    );
    let mut rules = topic_rules(&[]);
    rules.blocked.push(Blocked {
        id: Some("evil".to_string()),
        origin: None,
        reason: "malware".to_string(),
    });
    rules.blocked.push(Blocked {
        id: None,
        origin: Some("github.com/spammer/spam".to_string()),
        reason: "spam".to_string(),
    });
    let asked = Asked {
        net: &net,
        urls: RefCell::default(),
    };
    let got = run(&rules, &asked, &scratch);
    assert!(got.index.packages.is_empty(), "{}", said(&got));
    for expected in ["bad/evil: blocked, malware", "spammer/spam: blocked, spam"] {
        assert!(
            said(&got).contains(expected),
            "少了「{expected}」：\n{}",
            said(&got)
        );
    }
    let urls = asked.urls.borrow();
    assert!(
        !urls.iter().any(|url| url.contains("spammer/spam")),
        "拉黑的仓库连 Release 都不问：{urls:?}"
    );
}

#[test]
fn a_failed_search_still_publishes_the_official_packages() {
    let scratch = Scratch::new();
    let memes = package(scratch.path(), "memes", "1.0", "", None);
    for page in [None, Some(b"<html>rate limited</html>".to_vec())] {
        let mut net = Net::default();
        net.release("miyu-agent/miyu-memes", std::slice::from_ref(&memes));
        if let Some(body) = page {
            net.0.insert(page_url(1), body);
        }
        let mut rules = topic_rules(&["miyu-agent/miyu-memes"]);
        rules.sources.insert(0, Source::Official);
        let got = run(&rules, &net, &scratch);
        assert_eq!(got.index.packages.len(), 1, "{}", said(&got));
        assert!(got.index.packages[0].official);
        assert!(
            said(&got).contains("topic miyu-package: search failed, "),
            "{}",
            said(&got)
        );
    }
}

#[test]
fn paging_stops_on_a_short_page_and_at_ten_pages() {
    let scratch = Scratch::new();
    let full = |page: u32| -> Vec<serde_json::Value> {
        (0..100)
            .map(|n| {
                item(
                    &format!("p{page}/r{n}"),
                    true,
                    false,
                    "2026-10-01T00:00:00Z",
                )
            })
            .collect()
    };
    let mut net = Net::default();
    net.search_page(1, &full(1));
    net.search_page(2, &[item("p2/last", true, false, "2026-10-01T00:00:00Z")]);
    let asked = Asked {
        net: &net,
        urls: RefCell::default(),
    };
    let got = run(&topic_rules(&[]), &asked, &scratch);
    assert_eq!(
        asked
            .urls
            .borrow()
            .iter()
            .filter(|url| url.contains("/search/"))
            .count(),
        2,
        "第二页不满就停"
    );
    assert!(
        said(&got).contains("topic miyu-package: 101 repositories"),
        "{}",
        said(&got)
    );
    assert!(!said(&got).contains("search failed"), "{}", said(&got));

    let mut net = Net::default();
    for page in 1..=11 {
        net.search_page(page, &full(page));
    }
    let asked = Asked {
        net: &net,
        urls: RefCell::default(),
    };
    let got = run(&topic_rules(&[]), &asked, &scratch);
    assert!(
        !asked.urls.borrow().contains(&page_url(11)),
        "搜索最多给 1000 个，第 11 页不问"
    );
    assert!(
        said(&got).contains("topic miyu-package: stopped at 10 pages"),
        "{}",
        said(&got)
    );
}

#[test]
fn a_topic_that_github_would_not_accept_is_a_rule_error() {
    let scratch = Scratch::new();
    for (n, bad) in ["Miyu", "miyu package", "-miyu", "a&q=x"]
        .iter()
        .enumerate()
    {
        let dir = scratch.path().join(format!("bad-{n}"));
        put(
            &dir,
            "sources.toml",
            format!("[[source]]\nkind = \"topic\"\ntopic = \"{bad}\"\n").as_bytes(),
        );
        let error = Rules::read(&dir).expect_err("写错的报出来");
        assert!(error.contains("sources.toml"), "{bad}：{error}");
    }
}
