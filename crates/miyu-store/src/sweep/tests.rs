//! 不放过某一条（施工 V-43，[`super::Sweep::holding`]）：放得再勤，它和它以后的都留在有效历史里，前面的照旧放；不交的照旧
//! 放到最后几条。

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use super::{Plan, Settle, Sweep, Tuning};

/// 收下的序号。
#[derive(Default)]
struct Seqs(Vec<u64>);

impl Settle for Seqs {
    fn settle(&mut self, events: &[Event]) {
        self.0.extend(events.iter().map(|event| event.seq.get()));
    }
}

/// 一轮一轮：开头、人话、回复、结束，共 `n` 条。
fn events(n: u64) -> Vec<Event> {
    (1..=n)
        .map(|seq| {
            let at = "2026-11-04T00:00:00.000Z";
            let turn = (seq - 1) / 4 * 4 + 2;
            let line = match seq % 4 {
                1 => format!(r#"{{"seq":{seq},"at":"{at}","kind":"message.user","by":{{"kind":"person"}},"body":{{"blocks":[]}}}}"#),
                2 => format!(r#"{{"seq":{seq},"at":"{at}","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"body":{{"trigger":{}}}}}"#, seq - 1),
                3 => format!(r#"{{"seq":{seq},"at":"{at}","kind":"message.assistant","turn":{turn},"by":{{"kind":"model","endpoint":"e","model":"m"}},"body":{{"blocks":[],"seen":{}}}}}"#, seq - 1),
                _ => format!(r#"{{"seq":{seq},"at":"{at}","kind":"turn.ended","turn":{turn},"by":{{"kind":"kernel"}},"body":{{"reason":"completed"}}}}"#),
            };
            Event::from_line(&line).expect("合写法")
        })
        .collect()
}

fn sweep(hold: Option<u64>) -> (Seq, Vec<u64>, Vec<u64>) {
    let tuning = Tuning {
        flush_at: 8,
        keep: 2,
    };
    let mut sweep = Sweep::new(Plan::guess(), tuning, Seqs::default());
    if let Some(hold) = hold {
        sweep = sweep.holding(Seq::new(hold).expect("从 1 起"));
    }
    for event in events(200) {
        sweep.feed(vec![event]);
    }
    let from = sweep.from();
    let (tail, settled) = sweep.into_tail();
    let kept = tail.events().iter().map(|event| event.seq.get()).collect();
    (from, settled.0, kept)
}

#[test]
fn what_is_held_stays_and_the_rest_is_let_go() {
    let (from, settled, kept) = sweep(Some(150));
    assert!(
        from.get() <= 150 && from.get() > 100,
        "放到了第 150 条前面不远：{from:?}"
    );
    assert_eq!(
        settled,
        (1..from.get()).collect::<Vec<_>>(),
        "前面的照先后收下"
    );
    assert_eq!(
        kept,
        (from.get()..=200).collect::<Vec<_>>(),
        "它和它以后的都留着"
    );
    let (from, _, kept) = sweep(None);
    assert!(from.get() > 190, "不交的放到最后几条：{from:?}");
    assert_eq!(kept.last(), Some(&200));
}
