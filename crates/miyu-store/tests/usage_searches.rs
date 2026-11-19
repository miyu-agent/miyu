//! 供应商替模型运行的搜索、抓取的次数记进用量汇总（施工 DSR-7，设计 72 第二节第 4 条）：`model.called` 的用量里报了的，
//! 一行一行记下，查的时候照分组加起来；没报的那一组不带这两格；删掉的会话照样留着；老的表（没有这两列）删掉照日志重建。

use miyu_kernel::event::Event;
use miyu_store::sqlite::Opened;
use miyu_store::trash;
use miyu_store::usage::{Group, UsageIndex};

use crate::support::*;

/// 一次发出去的请求：用量里带着搜索 `searches` 次、抓取 `fetches` 次（0 的不写，同驱动交的）。
fn searched(seq: u64, minutes: i64, searches: u64, fetches: u64) -> Event {
    let count = |name: &str, n: u64| match n {
        0 => String::new(),
        n => format!(r#","{name}":{n}"#),
    };
    line(
        seq,
        at(minutes),
        "model.called",
        &format!(
            r#"{{"seen":{},"endpoint":"deepseek","model":"deepseek-flash","messages":1,"usage":{{"uncached":10,"cache_read":0,"cache_write":0,"output":2{}{}}},"result":"ok"}}"#,
            seq - 1,
            count("searches", searches),
            count("fetches", fetches),
        ),
    )
}

/// 两个会话：第一个搜索过（4 次搜索 1 次抓取，再 2 次搜索），第二个没有。
fn spoken(root: &miyu_store::root::DataRoot, index: &UsageIndex) {
    let first = session(1);
    let mut log = write_log(&root.session_dir(&first), &[]);
    let events = [
        created(at(0), None),
        said(2, at(1)),
        searched(3, 2, 4, 1),
        searched(4, 3, 2, 0),
    ];
    land(index, &mut log, &first, &who(None), &events);
    let second = session(2);
    let mut log = write_log(&root.session_dir(&second), &[]);
    let events = [
        created(at(10), None),
        said(2, at(11)),
        searched(3, 12, 0, 0),
    ];
    land(index, &mut log, &second, &who(None), &events);
}

fn counts(total: &miyu_store::usage::Total) -> (Option<u64>, Option<u64>) {
    (total.usage.searches, total.usage.fetches)
}

#[test]
fn searches_and_fetches_add_up_in_total_and_by_group() {
    let scratch = Scratch::new("usage-searches");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    let all = total(&index);
    assert_eq!(all.requests, 3);
    assert_eq!(counts(&all), (Some(6), Some(1)));
    assert_eq!((all.usage.uncached, all.usage.output), (30, 6), "四项不变");
    // 按会话：搜索过的那个带着，没搜索过的不带这两格。
    let by_session = totals(&index, &query(&[Group::Session]));
    assert_eq!(by_session.len(), 2);
    assert_eq!(counts(&by_session[0]), (Some(6), Some(1)));
    assert_eq!(counts(&by_session[1]), (None, None));
    // 按别的分组也加：同一个模型的三次请求一组。
    let by_model = totals(&index, &query(&[Group::Model]));
    assert_eq!(by_model.len(), 1);
    assert_eq!(counts(&by_model[0]), (Some(6), Some(1)));
    // 表删了照日志重建，数一样。
    drop(index);
    forget(&root);
    let (rebuilt, _) = UsageIndex::open(&root);
    assert_eq!(counts(&total(&rebuilt)), (Some(6), Some(1)));
}

#[test]
fn a_purged_session_keeps_its_counts() {
    let scratch = Scratch::new("usage-searches-purged");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    trash::discard(&root, &session(1), at(60)).unwrap();
    let week = std::time::Duration::from_secs(7 * 24 * 3600);
    let purged = trash::purge(&root, at(2000 + 8 * 24 * 60), week).expect("回收处读得了");
    assert_eq!(purged.removed, 1);
    assert_eq!(counts(&total(&index)), (Some(6), Some(1)));
    // 汇总删了重建：日志没了，照 `usage.purged` 里记的。
    drop(index);
    forget(&root);
    let (rebuilt, _) = UsageIndex::open(&root);
    let all = total(&rebuilt);
    assert_eq!(all.requests, 3);
    assert_eq!(counts(&all), (Some(6), Some(1)));
}

/// 上一版的表没有这两列、版本是 3：照派生库的规矩删掉重建（设计 07 第六节），次数照日志重新算出来。
#[test]
fn an_older_table_is_rebuilt_with_the_counts_from_the_logs() {
    let scratch = Scratch::new("usage-searches-older");
    let root = root_in(&scratch);
    let path = root.usage_file();
    {
        let (index, _) = UsageIndex::open(&root);
        spoken(&root, &index);
        assert_eq!(total(&index).requests, 3);
    }
    // 变回上一版的样子：没有这两列，版本是 3。
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(
            "ALTER TABLE spent DROP COLUMN searches; ALTER TABLE spent DROP COLUMN fetches; \
             PRAGMA user_version = 3;",
        )
        .unwrap();
    }
    let (index, opened) = UsageIndex::open(&root);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    let all = total(&index);
    assert_eq!(all.requests, 3);
    assert_eq!(counts(&all), (Some(6), Some(1)));
}
