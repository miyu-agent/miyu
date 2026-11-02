//! 上线时刻和下载次数（施工 DL-2）：沿用 `published`；下载次数照全部发布加、翻页、别的文件不算；`stats.json` 只留 8 天、
//! 近 7 天照 7 天前那天减；问不成的那个包照上一份沿用，上一份也没有的不写。网络换成内存里的一张表。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;

const API: &str = "https://api.example.invalid";

/// 内存里的网络：地址到字节，拿不到的报 404。
#[derive(Default)]
struct Net(BTreeMap<String, Vec<u8>>);

impl Fetch for Net {
    fn get(&self, url: &str, _limit: u64) -> Result<Vec<u8>, String> {
        self.0
            .get(url)
            .cloned()
            .ok_or_else(|| format!("{url}: 404"))
    }
}

impl Net {
    /// 仓库 `repo` 的第 `page` 页发布：每个发布几个文件（名字、下载次数）。
    fn page(mut self, repo: &str, page: usize, releases: &[&[(&str, u64)]]) -> Net {
        let releases: Vec<Value> = releases
            .iter()
            .map(|assets| {
                let assets: Vec<Value> = assets
                    .iter()
                    .map(|(name, count)| json!({"name": name, "download_count": count}))
                    .collect();
                json!({"assets": assets})
            })
            .collect();
        self.0.insert(
            format!("{API}/repos/{repo}/releases?per_page=100&page={page}"),
            serde_json::to_vec(&releases).expect("写得成"),
        );
        self
    }
}

/// 一份索引：生成时刻、几个包（编号、版本、`published`）。
fn index(generated: &str, packages: &[(&str, &str, Option<&str>)]) -> Index {
    let packages: Vec<Value> = packages
        .iter()
        .map(|(id, version, published)| {
            let mut entry = json!({"id": id, "version": version, "origin": format!("github.com/miyu-agent/miyu-{id}"), "files": []});
            if let Some(published) = published {
                entry["published"] = json!(published);
            }
            entry
        })
        .collect();
    let text = json!({"version": 1, "generated": generated, "packages": packages});
    Index::parse(&serde_json::to_vec(&text).expect("写得成")).expect("读得成")
}

fn published(index: &Index) -> Vec<(&str, Option<&str>)> {
    index
        .packages
        .iter()
        .map(|entry| (entry.id.as_str(), entry.published.as_deref()))
        .collect()
}

fn date(text: &str) -> Date {
    text.parse().expect("是日期")
}

#[test]
fn published_is_carried_over_while_the_version_stays() {
    const NOW: &str = "2026-11-02T03:00:00Z";
    let previous = index(
        "2026-11-01T03:00:00Z",
        &[
            ("draw", "1.0", Some("2026-10-20T03:00:00Z")),
            ("memes", "1.0", Some("2026-10-21T03:00:00Z")),
            ("plain", "2.0", None),
        ],
    );
    let mut now = index(
        NOW,
        &[
            ("draw", "1.0", None),
            ("memes", "1.1", None),
            ("new", "0.1", None),
            ("plain", "2.0", None),
        ],
    );
    carry_published(&mut now, Some(&previous));
    assert_eq!(
        published(&now),
        [
            ("draw", Some("2026-10-20T03:00:00Z")),
            ("memes", Some(NOW)),
            ("new", Some(NOW)),
            ("plain", Some(NOW)),
        ],
        "版本没变的照抄；升了版、新包、上一份没有这一格的写这一次的时刻"
    );
    let mut first = index(NOW, &[("draw", "1.0", Some("2026-10-20T03:00:00Z"))]);
    carry_published(&mut first, None);
    assert_eq!(published(&first), [("draw", Some(NOW))], "没有上一份的");
}

#[test]
fn downloads_add_up_every_release_and_every_target() {
    let full: Vec<&[(&str, u64)]> = vec![&[("memes-0.1.miyu", 1)]; 100];
    let net = Net::default()
        .page(
            "miyu-agent/miyu-memes",
            1,
            &[
                &[
                    ("memes-1.0-linux-x86_64.miyu", 10),
                    ("memes-1.0-windows-x86_64.miyu", 20),
                    ("memes-1.0.miyu.sig", 1000),
                    ("memes-extra-1.0.miyu", 1000),
                    ("README.md", 1000),
                ],
                &[("memes-0.9.miyu", 5)],
                &[],
            ],
        )
        .page("miyu-agent/miyu-draw", 1, &full)
        .page("miyu-agent/miyu-draw", 2, &[&[("draw-0.1.miyu", 7)]]);
    let memes = downloads(&net, API, "github.com/miyu-agent/miyu-memes", "memes");
    assert_eq!(memes, Ok(35), "各版本、各系统加起来，别的文件不算");
    let draw = downloads(&net, API, "github.com/miyu-agent/miyu-draw", "draw");
    assert_eq!(draw, Ok(7), "满一页的翻下一页（第一页都不是它的）");
    assert!(downloads(&net, API, "gitlab.com/a/b", "memes").is_err());
    assert!(downloads(&net, API, "github.com/miyu-agent/gone", "gone").is_err());
}

#[test]
fn stats_keep_eight_days_and_the_week_is_today_minus_seven_days_ago() {
    let mut stats = Stats::parse(
        br#"{"version": 1, "days": [
            {"date": "2026-10-24", "downloads": {"memes": 1}},
            {"date": "2026-10-25", "downloads": {"memes": 10, "draw": 50}},
            {"date": "2026-11-01", "downloads": {"memes": 30}},
            {"date": "2026-11-02", "downloads": {"memes": 999}},
            {"date": "2026-11-09", "downloads": {"memes": 999}},
            {"date": "someday", "downloads": {"memes": 999}}
        ]}"#,
    )
    .expect("读得成");
    let today = date("2026-11-01");
    stats.record(
        today,
        BTreeMap::from([("memes".to_string(), 42), ("draw".to_string(), 40)]),
    );
    let dates: Vec<&str> = stats.days.iter().map(|day| day.date.as_str()).collect();
    assert_eq!(
        dates,
        ["2026-10-25", "2026-11-01"],
        "8 天以前的、同一天的旧数、比今天晚的、写错的都去掉"
    );
    assert_eq!(stats.week(today, "memes"), Some(32));
    assert_eq!(stats.week(today, "draw"), Some(0), "少了的是 0");
    assert_eq!(stats.week(today, "new"), None, "7 天前没有它的");
    assert_eq!(stats.week(date("2026-11-02"), "memes"), None, "那天没记的");
    assert_eq!(Stats::parse(br#"{"version": 2, "days": []}"#), None);
    assert_eq!(Stats::parse(b"not json"), None);
}

#[test]
fn counting_writes_both_fields_and_keeps_the_last_count_when_asking_fails() {
    let net = Net::default()
        .page("miyu-agent/miyu-memes", 1, &[&[("memes-1.0.miyu", 12)]])
        .page("miyu-agent/miyu-draw", 1, &[&[("draw-1.0.miyu", 3)]]);
    let mut stats = Stats::parse(
        br#"{"version": 1, "days": [{"date": "2026-10-26", "downloads": {"memes": 2, "gone": 1}}]}"#,
    )
    .expect("读得成");
    let mut previous = index(
        "2026-11-01T03:00:00Z",
        &[
            ("gone", "0.9", None),
            ("lost", "1.0", None),
            ("memes", "1.0", None),
        ],
    );
    previous.packages[0].downloads = Some(5);
    previous.packages[0].downloads_week = Some(4);
    previous.packages[2].downloads = Some(11);
    let mut now = index(
        "2026-11-02T03:00:00Z",
        &[
            ("draw", "1.0", None),
            ("gone", "1.0", None),
            ("lost", "1.0", None),
            ("memes", "1.0", None),
        ],
    );
    let notes = count(
        &mut now,
        Some(&previous),
        &mut stats,
        date("2026-11-02"),
        (&net, API),
    );
    let counted: Vec<(&str, Option<u64>, Option<u64>)> = now
        .packages
        .iter()
        .map(|entry| (entry.id.as_str(), entry.downloads, entry.downloads_week))
        .collect();
    assert_eq!(
        counted,
        [
            ("draw", Some(3), None),
            ("gone", Some(5), Some(4)),
            ("lost", None, None),
            ("memes", Some(12), Some(10))
        ],
        "问成的照这一次；问不成的照上一份同编号的沿用（不写成 0），上一份也没有的不写"
    );
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(
        notes[0].starts_with("gone: downloads kept from the last index"),
        "{notes:?}"
    );
    assert!(
        notes[1].starts_with("lost: downloads left out"),
        "{notes:?}"
    );
    assert_eq!(
        stats.days.last().map(|day| day.downloads.clone()),
        Some(BTreeMap::from([
            ("draw".to_string(), 3),
            ("memes".to_string(), 12)
        ])),
        "沿用的不记进今天"
    );
    let mut alone = index("2026-11-02T03:00:00Z", &[("lost", "1.0", None)]);
    let notes = count(
        &mut alone,
        None,
        &mut stats,
        date("2026-11-02"),
        (&net, API),
    );
    assert_eq!(
        (alone.packages[0].downloads, notes.len()),
        (None, 1),
        "没有上一份的"
    );
}

/// 审计（2026-11-02 第三轮）：`download_count` 是 GitHub（或 `MIYU_INDEX_API` 指的地方）回的数，加起来不该溢出：调试版
/// 直接 panic，发布版（没开 overflow-checks）悄悄绕回一个小数写进索引。期望饱和到 `u64::MAX` 或者报错，不 panic。
#[test]
fn audit_download_counts_do_not_overflow() {
    let big = u64::MAX / 2 + 1;
    let net = Net::default().page(
        "miyu-agent/miyu-memes",
        1,
        &[&[("memes-1.0.miyu", big)], &[("memes-1.1.miyu", big)]],
    );
    let counted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        downloads(&net, API, "github.com/miyu-agent/miyu-memes", "memes")
    }));
    assert!(
        matches!(counted, Ok(Ok(u64::MAX) | Err(_))),
        "两个 2^63 的下载次数加起来溢出了：{:?}",
        counted.as_ref().map_err(|_| "panic")
    );
}
