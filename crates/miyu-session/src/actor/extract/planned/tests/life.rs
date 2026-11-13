//! 人生会话的抽取（施工 LF-3，`memory.md` 第六条第 3、4 款）：人生循环叫醒的回合算答了的一轮，交进去的一块里只有她的回答
//! （`You:` 那一行），没有叫醒的那段话；人在人生会话里说的一轮照私聊的写法（`Owner:`），两种混着照日志的先后。

use std::path::Path;

use miyu_store::log::SessionLog;

use super::{Fixture, Log, MAIN, assembler, scratch};
use crate::memory::{ExtractTexts, Plan};
use miyu_kernel::id::SessionId;

/// 叫醒的那段话：每次都一样的推演方法。
const WAKE: &str = "推演这一段时间里你过了什么";

/// 人生会话是本机的私聊会话：私聊的组装器。
fn private(name: &str) -> Fixture {
    let scratch = scratch(name);
    let assembler = assembler(&scratch, false);
    Fixture {
        dir: scratch.0.join("session"),
        _scratch: scratch,
        assembler,
        texts: ExtractTexts::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
            .expect("读得到"),
        own: SessionId::parse(MAIN).expect("会话编号合写法"),
    }
}

impl Log {
    /// 人生循环（任务 `t1`）叫醒她一次。
    fn woke(&mut self, looping: &str) {
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.user","by":{{"kind":"schedule","schedule":"t1","loop":"{looping}"}},"cause":"s-{seq}","body":{{"blocks":[{{"type":"text","text":"{WAKE}"}}]}}}}"#
            )
        });
    }

    /// 主人在本机的头上说一句。
    fn owner_here(&mut self, text: &str) {
        self.push(|seq, at| {
            format!(
                r#"{{"seq":{seq},"at":"{at}","kind":"message.user","by":{{"kind":"person"}},"cause":"o-{seq}","body":{{"blocks":[{{"type":"text","text":"{text}"}}]}}}}"#
            )
        });
    }
}

/// 照私聊会话（没有群聊近况的预算）整份读拼。
fn asked(fixture: &Fixture, min_turns: usize) -> String {
    let mut planning = fixture.planning();
    planning.budget = None;
    planning.limit = None;
    planning.min_turns = min_turns;
    let events = miyu_store::log::read_events(&fixture.dir).expect("读得了");
    match super::planned(
        crate::memory::Read {
            after: miyu_kernel::id::Seq::new(1).expect("从 1 起"),
            events,
            from: crate::memory::ReadFrom::Start(None),
        },
        &planning,
    ) {
        Plan::Ask { text, .. } => text,
        other => panic!("要发：{other:?}"),
    }
}

#[test]
fn turns_the_life_loop_woke_are_answered_turns_with_only_her_answer() {
    let fixture = private("life-only");
    let mut log = SessionLog::create(&fixture.dir, 1 << 20).expect("建得了日志");
    let mut events = Log::new();
    events.woke("life");
    let first = events.answer("上午去菜市场买了条鱼。", false) - 1;
    events.woke("life");
    let second = events.answer("下午把鱼炖了，邻居来蹭饭。", false) - 1;
    log.append(&events.take()).expect("写得进");
    let text = asked(&fixture, 2);
    assert!(
        text.ends_with(&format!(
            "<turn number=\"{first}\" date=\"2026-10-21\">\nYou: 上午去菜市场买了条鱼。\n</turn>\n\n<turn number=\"{second}\" date=\"2026-10-21\">\nYou: 下午把鱼炖了，邻居来蹭饭。\n</turn>"
        )),
        "攒够两轮照样发，每一轮只有她的回答：{text}"
    );
    assert!(!text.contains(WAKE), "没有叫醒的那段话：{text}");
    assert!(
        !text.contains("\nOwner:"),
        "叫醒的那段话不当成主人说的：{text}"
    );
}

#[test]
fn what_the_owner_said_and_what_the_life_loop_woke_keep_their_own_shape_in_order() {
    let fixture = private("life-mixed");
    let mut log = SessionLog::create(&fixture.dir, 1 << 20).expect("建得了日志");
    let mut events = Log::new();
    events.woke("life");
    let woke = events.answer("早上去河边跑了步。", false) - 1;
    events.owner_here("你今天干嘛了");
    let asked_turn = events.answer("跑步，然后发呆。", false) - 1;
    events.woke("life");
    let again = events.answer("傍晚在阳台看云。", false) - 1;
    log.append(&events.take()).expect("写得进");
    let text = asked(&fixture, 1);
    assert!(
        text.ends_with(&format!(
            "<turn number=\"{woke}\" date=\"2026-10-21\">\nYou: 早上去河边跑了步。\n</turn>\n\n<turn number=\"{asked_turn}\" date=\"2026-10-21\">\nOwner: 你今天干嘛了\nYou: 跑步，然后发呆。\n</turn>\n\n<turn number=\"{again}\" date=\"2026-10-21\">\nYou: 傍晚在阳台看云。\n</turn>"
        )),
        "{text}"
    );
    assert!(!text.contains(WAKE), "{text}");
}
