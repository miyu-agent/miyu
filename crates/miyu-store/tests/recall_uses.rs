//! 回合库里的「用到」（施工 R-4 下，`docs/blueprint/recall.md`「库的结构」、`memory.md` 第八条）：一条事件带上一条记忆写一行，
//! 同一行再写不重复；照记忆分组数次数、取最近一次；删会话时跟着那个来源拿掉。

use std::path::PathBuf;

use miyu_kernel::id::Seq;
use miyu_store::recall::{Edit, RecallIndex, Used};

use crate::support::*;

fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch)
        .index(&admin())
        .join("recall")
        .join("turns-test.db")
}

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("序号从 1 起")
}

fn used(key: &str, memory: &str, minute: i64) -> Edit {
    Edit::Use {
        key: key.into(),
        memory: memory.into(),
        at: at(minute),
    }
}

#[test]
fn uses_count_once_per_event_and_go_with_their_session() {
    let scratch = Scratch::new("recall-uses");
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert!(index.uses().expect("读得了").is_empty());
    index
        .apply(
            "s1",
            &[
                used("s1/3", "m1", 1),
                used("s1/3", "m2", 1),
                used("s1/7", "m1", 5),
            ],
            seq(7),
        )
        .expect("写得进");
    index
        .apply("s1", &[used("s1/7", "m1", 5)], seq(7))
        .expect("写得进");
    index
        .apply("s2", &[used("s2/2", "m1", 3)], seq(2))
        .expect("写得进");
    let uses = index.uses().expect("读得了");
    assert_eq!(
        uses.get("m1"),
        Some(&Used {
            count: 3,
            last: at(5)
        }),
        "同一条事件再写不重复，最近一次是最晚的那一条"
    );
    assert_eq!(
        uses.get("m2"),
        Some(&Used {
            count: 1,
            last: at(1)
        })
    );

    index.forget("s1").expect("拿得掉");
    let uses = index.uses().expect("读得了");
    assert_eq!(
        uses.get("m1"),
        Some(&Used {
            count: 1,
            last: at(3)
        }),
        "删掉的会话里的不算"
    );
    assert!(!uses.contains_key("m2"));

    // 重开的库还在；坏了重建的空了，照日志补。
    drop(index);
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert_eq!(index.uses().expect("读得了").len(), 1);
    index.reset().expect("建得成");
    assert!(index.uses().expect("读得了").is_empty());
}
