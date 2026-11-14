//! 留着编好的历史（施工 V-42，[`Memo`]）：拿整份编的（[`Driver::fragments`]，改前的写法）当准绳，照种子造几千段对话，每一次
//! 请求两边编出来的片段清单、每条消息的位置、路径（编不了的报的错）都要一模一样。
//!
//! 每段对话一份 `Memo`，像一个会话那样一次次请求下去，中间随机来：加 user（字、图、PDF、文本文件、别的文件）、assistant（正文、
//! 各家的思考、各家编号的工具调用，偶尔编号重了，供应商侧工具）和工具结果（字、夹图、出错、没输出）；同一轮里连发几次（缓存命中）；撤销、
//! 压缩、改前面某一条、往最后那条 user 里插事实块、接着写、接上 respond 带的那一句（施工 RT-1）；换 system、工具面、稳定区、模型、输出上限、温度、思考强度、模型能收
//! 什么、图的转述、占位的几句（retext）、驱动和开关（含思考回传的几种）；blob 没了、又回来了、换了同名不同长短的。每一种都有
//! 专门的一段把它单独来几遍。
//!
//! 平时只跑少量种子（CI 的 Windows debug 下几秒）；全量：`MIYU_MEMO_SEEDS=<段数>`，见施工单 `V-42`。

mod change;
mod talk;

use miyu_drivers::Memo;

use change::{CHANGES, Change};
use talk::{DRIVERS, Talk};

/// 平时跑几段：CI 的 Windows debug 下要快。
const QUICK: u64 = 24;

/// 每段多少次请求。
const STEPS: usize = 70;

/// 跑一段的结果：编不了的次数；留着的、接着用了的条数（加起来）。
#[derive(Default)]
struct Tally {
    failed: usize,
    kept: usize,
    reused: usize,
}

/// 跑一段：一份 `Memo` 一路请求下去，每次都比。`focus` 有的，平时的改动之外多来这一种。
fn run(seed: u64, focus: Option<Change>, tally: &mut Tally) {
    let mut talk = Talk::new(seed);
    let mut memo = Memo::new();
    for step in 0..STEPS {
        let change = match focus {
            Some(focus) if talk.rng.chance(35) => focus,
            _ if talk.rng.chance(55) => Change::Turn,
            _ if talk.rng.chance(40) => Change::Again,
            _ => *talk.rng.pick(&CHANGES),
        };
        talk.apply(change);
        tally.kept += memo.len();
        if talk.compare(&mut memo, seed, step, &format!("{change:?}")) {
            tally.failed += 1;
        }
        tally.reused += memo.reused();
        // 同一轮里再发一次：什么都没变，留着的整份都该接着用。
        if talk.rng.chance(30) {
            let before = memo.len();
            if !talk.compare(&mut memo, seed, step, "再发一次") {
                assert_eq!(
                    memo.reused(),
                    before,
                    "种子 {seed} 第 {step} 次：再发一次整份接着用"
                );
            }
        }
    }
}

/// 要跑几段：平时 [`QUICK`]，设了 `MIYU_MEMO_SEEDS` 的照它。
fn seeds() -> u64 {
    std::env::var("MIYU_MEMO_SEEDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(QUICK)
}

#[test]
fn kept_history_encodes_the_same_bytes_as_encoding_it_whole() {
    let mut tally = Tally::default();
    for seed in 0..seeds() {
        run(seed, None, &mut tally);
    }
    // 缺 blob 的也比过了：编不了的两边报同一个。
    assert!(tally.failed > 0, "这几段里有编不了的请求，两边报的一样");
    // 真的在接着用：留着的里面大半接着用了（这几段改动来得比真的会话勤得多）。
    assert!(
        tally.reused * 2 > tally.kept,
        "接着用了 {} 条，留着过 {} 条",
        tally.reused,
        tally.kept
    );
}

#[test]
fn each_kind_of_change_on_its_own_keeps_the_bytes_the_same() {
    let per = (seeds() / 4).max(2);
    for (index, change) in CHANGES.iter().enumerate() {
        for seed in 0..per {
            run(
                1_000_000 + (index as u64) * 10_000 + seed,
                Some(*change),
                &mut Tally::default(),
            );
        }
    }
}

/// 一段长的：同一个驱动一直说下去，留着的条数跟着长（确认真的在接着用，不是每次都从头编）。
#[test]
fn a_long_talk_keeps_reusing_the_history() {
    for driver in 0..DRIVERS {
        long_talk(driver);
    }
}

fn long_talk(driver: usize) {
    let mut talk = Talk::new(7);
    talk.driver = driver;
    talk.repeats = false;
    let mut memo = Memo::new();
    for step in 0..60 {
        let before = memo.len();
        talk.apply(Change::Turn);
        talk.compare(&mut memo, 7, step, "长对话");
        assert_eq!(
            memo.reused(),
            before,
            "第 {step} 次：上一次留着的都接着用了"
        );
        assert_eq!(
            memo.len(),
            talk.request.messages.len(),
            "第 {step} 次：每条消息都留着"
        );
    }
}
