//! 带上限的列法（施工 YC-13）：列出来的就是不带上限的头几个，一字不差；从索引里读的整行跟着上限走，不跟着会话总数走。
//! 量尺在最后，只量不断言，跑法：`cargo test --release -p miyu-endpoint --lib limited -- --ignored --nocapture`。

use std::fs;
use std::time::Instant;

use super::indexed::{created, index_path, lines, meta, said, turn, varied, write};
use super::*;

/// 第 `n` 个会话的编号：`n` 越大越新。
fn id(n: u64) -> String {
    format!("0192f3a1-{n:04x}-7abc-8def-001122334455")
}

/// 带用量的一次模型调用：这个会话自己的 token 多 `tokens`。
fn called(seq: u64, tokens: u64) -> String {
    json!({"seq": seq, "at": "2026-09-25T08:00:00.000Z", "kind": "model.called", "turn": 2, "by": {"kind": "kernel"}, "body": {"seen": 1, "messages": 2, "result": "ok", "usage": {"uncached": tokens, "cache_read": 0, "cache_write": 0, "output": 0}}}).to_string()
}

/// 往 `site` 里写 `count` 个会话：每 7 个里一个是前一个的子会话，每 10 个里一个是一次性的，每 13 个里一个是通讯平台的
/// 场所会话，每 5 个里一个改过名、置顶；个个用过 token。
fn crowd(site: &Site, count: u64) {
    for n in 0..count {
        let parent = (n % 7 == 3).then(|| id(n - 1));
        let mut first = created(n % 10 == 0, parent.as_deref());
        if n % 13 == 0 {
            first = first.replace(r#""venue":"local""#, r#""venue":"qq:group:1""#);
        }
        let mut log = vec![first, said(2), called(3, n + 1)];
        if n % 5 == 0 {
            log.push(meta(4, Some("起了名"), Some(true)));
            log.push(turn(5, Some("~/b")));
        }
        write(site, &id(n), "000000000001.jsonl", &lines(&log));
    }
}

/// 怎么挑。
type Pick = fn(&Row) -> bool;

/// 列一次，写成 `session.list` 的回应那样（连同子孙的 token、默认工作区都填了）。
fn listing(site: &Site, index: &SessionIndex, pick: Pick, limit: Option<usize>) -> Vec<Value> {
    let busy = BTreeSet::from([SessionId::parse(B).expect("合写法")]);
    let listed = gather(&site.1, index, &busy, pick, limit).expect("读得了");
    listed.iter().map(Listed::to_json).collect()
}

/// 几种挑法：全要、只要一次性的、只要本机的主会话、只要通讯平台的。
const PICKS: [(&str, Pick); 4] = [
    ("全要", |_| true),
    ("一次性的", |row| row.oneshot),
    ("本机的主会话", |row| {
        local(row) && row.parent.is_none()
    }),
    ("通讯平台的", |row| !local(row)),
];

/// 带上限的每一种个数，都是不带上限的头几个。
fn check(site: &Site, index: &SessionIndex, limits: &[usize], said: &str) {
    for (name, pick) in PICKS {
        let whole = listing(site, index, pick, None);
        for &limit in limits {
            let most = limit.min(whole.len());
            assert_eq!(
                listing(site, index, pick, Some(limit)),
                whole[..most],
                "{said}，{name}，上限 {limit}"
            );
        }
    }
}

#[test]
fn a_limited_listing_is_the_head_of_the_whole_one() {
    // 各种各样的会话（坏了的、两段的、空目录……），再加一个通讯平台的。
    let site = varied();
    let venue = created(false, None).replace(r#""venue":"local""#, r#""venue":"qq:group:1""#);
    write(&site, &id(1), "000000000001.jsonl", &lines(&[venue]));
    let limits: Vec<usize> = (0..=9).collect();
    // 索引是空的，带上限的先列：只填了列到的那几行，后面的照样对。
    for &limit in &limits {
        let fresh = varied();
        let (index, _) = SessionIndex::open(&index_path(&fresh));
        let (first, _) = PICKS[0];
        let head = listing(&fresh, &index, PICKS[0].1, Some(limit));
        let whole = listing(&fresh, &index, PICKS[0].1, None);
        assert_eq!(
            head,
            whole[..limit.min(whole.len())],
            "空索引，{first}，上限 {limit}"
        );
    }
    let (index, _) = SessionIndex::open(&index_path(&site));
    check(&site, &index, &limits, "照日志填");
    check(&site, &index, &limits, "照索引读");
}

#[test]
fn a_limited_listing_crosses_batches_and_gaps_in_the_index() {
    let site = Site::new(&[]);
    crowd(&site, 200);
    let (index, _) = SessionIndex::open(&index_path(&site));
    let limits = [0, 1, 5, 31, 32, 33, 63, 64, 65, 150, 199, 200, 201];
    check(&site, &index, &limits, "填好的索引");
    // 索引里有、目录没了的（删了的会话），目录里有、索引没有的（新的、行丢了的），日志比索引长的。
    for n in [199, 170, 100, 3] {
        let dir = site
            .1
            .session_dir(&SessionId::parse(&id(n)).expect("合写法"));
        fs::remove_dir_all(dir).expect("删得掉");
    }
    for n in [198, 140, 33] {
        index
            .remove(&SessionId::parse(&id(n)).expect("合写法"))
            .expect("删得掉");
    }
    for n in 200..203 {
        write(
            &site,
            &id(n),
            "000000000001.jsonl",
            &lines(&[created(false, None), said(2)]),
        );
    }
    let longer = lines(&[
        created(false, None),
        said(2),
        called(3, 197),
        meta(4, Some("后来"), None),
    ]);
    write(&site, &id(196), "000000000001.jsonl", &longer);
    // 先带上限列（索引还缺着），再对整份的。
    let all = PICKS[0].1;
    let head = listing(&site, &index, all, Some(40));
    let truth = scan(&site.1, None, &BTreeSet::new(), all, None, &Stop::default()).expect("读得了");
    let ids = |listed: &[Value]| -> Vec<String> {
        listed
            .iter()
            .map(|one| one["session"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    let truth: Vec<Value> = truth.iter().map(Listed::to_json).collect();
    assert_eq!(
        ids(&head),
        ids(&truth)[..40],
        "和不读索引的同一批、同一个先后"
    );
    assert_eq!(head, listing(&site, &index, all, None)[..40]);
    check(&site, &index, &limits, "缺了几行的索引");
}

#[test]
fn a_limited_listing_reads_rows_by_the_limit_not_by_how_many_sessions_there_are() {
    let site = Site::new(&[]);
    crowd(&site, 200);
    let (index, _) = SessionIndex::open(&index_path(&site));
    let whole = listing(&site, &index, PICKS[0].1, None);
    assert_eq!(whole.len(), 200);
    LOADED.set(0);
    let head = listing(&site, &index, PICKS[0].1, Some(5));
    assert_eq!(head, whole[..5]);
    let loaded = LOADED.get();
    assert!(loaded > 0, "读了索引");
    assert!(
        loaded <= 50,
        "200 个会话里列 5 个，从索引里读了 {loaded} 整行"
    );
}

fn median(mut runs: Vec<f64>) -> f64 {
    runs.sort_by(f64::total_cmp);
    runs[runs.len() / 2]
}

#[test]
#[ignore = "量尺，只量不断言"]
fn how_long_listing_fifty_of_two_thousand_sessions_takes() {
    for count in [500, 2000, 5000] {
        let site = Site::new(&[]);
        crowd(&site, count);
        let (index, _) = SessionIndex::open(&index_path(&site));
        let local_only: Pick = local;
        let once = |limit: Option<usize>| {
            let started = Instant::now();
            let listed = listing(&site, &index, local_only, limit);
            assert!(!listed.is_empty());
            started.elapsed().as_secs_f64() * 1000.0
        };
        println!("{count} 个会话，照日志填索引：{:.1} 毫秒", once(None));
        let limited = median((0..20).map(|_| once(Some(50))).collect());
        let whole = median((0..5).map(|_| once(None)).collect());
        println!("{count} 个会话：上限 50 中位 {limited:.2} 毫秒，全部 中位 {whole:.1} 毫秒");
    }
}
