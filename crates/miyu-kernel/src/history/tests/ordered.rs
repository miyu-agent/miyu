//! 照请求排（施工 V-17 下）：改成按连续的段直接写进一份以后，和原来先分段再拼的写法排得一样。造一长串交错的历史（请求在路上
//! 时来的话、乱序回来的结果、一次几个调用），每追加一条都比一次。

use super::*;

/// 原来的写法（V-17 下以前）：先把事件分进各段，每段照 [`reply_first`] 排，再拼起来。
fn reference(history: &History) -> Vec<u64> {
    let seen: Vec<Seq> = history
        .events
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(reply.seen),
            _ => None,
        })
        .collect();
    let mut segments: Vec<Vec<&Event>> = vec![Vec::new(); seen.len() + 1];
    for event in &history.events {
        let segment = seen.partition_point(|&boundary| boundary < event.seq);
        segments[segment].push(event);
    }
    segments
        .into_iter()
        .flat_map(|segment| {
            let Some(reply) = segment
                .iter()
                .copied()
                .find(|event| matches!(event.body, Body::MessageAssistant(_)))
            else {
                return segment;
            };
            let result_of_reply = |event: &Event| match &event.body {
                Body::ToolResult(result) if result.call_id.message() == reply.seq => {
                    Some(result.call_id.index())
                }
                _ => None,
            };
            let mut results: Vec<&Event> = segment
                .iter()
                .copied()
                .filter(|event| result_of_reply(event).is_some())
                .collect();
            results.sort_by_key(|event| result_of_reply(event));
            let rest = segment
                .iter()
                .copied()
                .filter(|event| event.seq != reply.seq && result_of_reply(event).is_none());
            std::iter::once(reply).chain(results).chain(rest).collect()
        })
        .map(|event| event.seq.get())
        .collect()
}

/// 定死种子的伪随机数（线性同余）：每次跑造出来的一样。
struct Dice(u64);

impl Dice {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

#[test]
fn ordering_by_runs_matches_ordering_by_segments() {
    let mut dice = Dice(17);
    let mut events = vec![created()];
    let mut seq = 1;
    let mut next = || {
        seq += 1;
        seq
    };
    for _ in 0..60 {
        let trigger = next();
        events.push(message(trigger, ALICE));
        let start = next();
        let body = format!(r#"{{"trigger":{trigger}}}"#);
        events.push(event(start, Some(start), KERNEL, "turn.started", &body));
        for step in 0..=dice.below(3) {
            let last = step == 3 || dice.below(3) == 0;
            let seen = events.last().map_or(1, |event| event.seq.get());
            // 请求在路上时人又说了一句：回复没看到它。
            if dice.below(4) == 0 {
                events.push(message(next(), ALICE));
            }
            let reply = next();
            let n = if last { 0 } else { 1 + dice.below(3) as u32 };
            events.push(event(
                reply,
                Some(start),
                MODEL,
                "message.assistant",
                &calls(reply, seen, n),
            ));
            // 结果乱序回来，中间夹着人说的话。
            let mut order: Vec<u32> = (1..=n).collect();
            for i in (1..order.len()).rev() {
                order.swap(i, dice.below(i as u64 + 1) as usize);
            }
            for k in order {
                if dice.below(5) == 0 {
                    events.push(message(next(), ALICE));
                }
                events.push(result(next(), start, &format!("call_{reply}_{k}")));
            }
            if last {
                break;
            }
        }
        let end = next();
        events.push(event(
            end,
            Some(start),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ));
    }
    let mut ledger = crate::ledger::Ledger::default();
    let mut history = History::default();
    for event in events {
        ledger.append(&event).unwrap();
        history.append(event);
        assert_eq!(ordered(&history), reference(&history));
    }
    assert!(history.events().len() > 300, "{}", history.events().len());
}
