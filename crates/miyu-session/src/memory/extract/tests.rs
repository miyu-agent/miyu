//! 抽取怎么拼这一段（`plan`）、一个会话的抽取状态（`Extractor`）：几轮怎么接、够不够、她记过的跳过、请求的字一个字一个字地
//! 对、放不下的分几次、一轮自己就放不下的截了中间也抽；闹钟作废、在路上的只有一个、同一段失败三次才放过。

use std::path::Path;

use miyu_kernel::time::Timestamp;

use super::*;

pub(super) fn texts() -> ExtractTexts {
    ExtractTexts::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
        .expect("读得到")
}

pub(super) fn seq(n: u64) -> Seq {
    Seq::new(n).expect("从 1 起")
}

fn at() -> Timestamp {
    Timestamp::parse("2026-10-09T03:00:00.000Z").expect("合写法")
}

/// 一轮：人在第 `said` 条说 `asked`，她在第 `said + 2` 条答 `answer`（回合从第 `said + 1` 条开始）。
fn exchange(said: u64, asked: &str, answer: &str) -> [Spoken; 2] {
    [
        Spoken {
            seq: seq(said),
            turn: None,
            at: at(),
            assistant: false,
            text: asked.to_string(),
        },
        Spoken {
            seq: seq(said + 2),
            turn: Some(TurnId::new(seq(said + 1))),
            at: at(),
            assistant: true,
            text: answer.to_string(),
        },
    ]
}

/// 人这边每一句都是主人说的。
fn owner(spoken: &[Spoken]) -> Voices {
    Voices {
        said: spoken
            .iter()
            .filter(|one| !one.assistant)
            .map(|one| (one.seq, Voice::Owner))
            .collect(),
        budget: None,
        limit: None,
        names: Default::default(),
        me: Default::default(),
        places: Vec::new(),
    }
}

pub(super) fn ask(plan: Plan) -> (Seq, String, BTreeSet<u64>, bool) {
    match plan {
        Plan::Ask {
            upto,
            text,
            turns,
            more,
            ..
        } => (upto, text, turns, more),
        other => panic!("要发：{other:?}"),
    }
}

/// 没有称呼的抽取：这里的几条不管主人怎么写（施工 R-6 再修另有一条）。
pub(super) fn plan(
    spoken: &[Spoken],
    remembered: &BTreeSet<TurnId>,
    min_turns: usize,
    offset: UtcOffset,
    texts: &ExtractTexts,
    voices: &Voices,
) -> Plan {
    super::plan(spoken, remembered, min_turns, offset, texts, voices, None)
}

/// 没有称呼时的指令。
fn instruction() -> String {
    let texts = texts();
    let owner = super::super::owner::owner(None, &texts.unnamed);
    say(&texts.instruction, &[("owner", &owner)])
}

#[test]
fn two_turns_are_written_after_the_instruction_turn_by_turn() {
    let spoken: Vec<Spoken> = [
        exchange(2, "我养了一只猫", "好的。"),
        exchange(6, "它叫团子", "记住了。"),
    ]
    .concat();
    let (upto, text, turns, more) = ask(plan(
        &spoken,
        &BTreeSet::new(),
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ));
    assert_eq!(upto, seq(8));
    assert_eq!(turns, [3, 7].into_iter().collect());
    assert!(!more);
    let instruction = instruction();
    assert_eq!(
        text,
        format!(
            "{}\n<turn number=\"3\" date=\"2026-10-09\">\nOwner: 我养了一只猫\nYou: 好的。\n</turn>\n\n<turn number=\"7\" date=\"2026-10-09\">\nOwner: 它叫团子\nYou: 记住了。\n</turn>",
            instruction.trim_end()
        )
    );
}

#[test]
fn too_few_answered_turns_wait_and_an_unanswered_line_is_left_for_later() {
    let one = exchange(2, "一", "答一");
    assert_eq!(
        plan(
            &one,
            &BTreeSet::new(),
            2,
            UtcOffset::UTC,
            &texts(),
            &owner(&one)
        ),
        Plan::Wait
    );
    assert_eq!(
        plan(
            &[],
            &BTreeSet::new(),
            1,
            UtcOffset::UTC,
            &texts(),
            &Voices::default()
        ),
        Plan::Wait
    );
    let mut spoken = [exchange(2, "一", "答一"), exchange(6, "二", "答二")].concat();
    spoken.push(Spoken {
        seq: seq(10),
        turn: None,
        at: at(),
        assistant: false,
        text: "还没答的".to_string(),
    });
    let (upto, text, _, _) = ask(plan(
        &spoken,
        &BTreeSet::new(),
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ));
    assert_eq!(upto, seq(8), "抽到答了的最后一轮");
    assert!(!text.contains("还没答的"));
}

#[test]
fn only_the_turns_where_she_remembered_are_left_out() {
    // 施工 E-5：她在第二轮当场记过，那一轮不交，第一、三轮照抽，抽到最后一轮。
    let spoken = [
        exchange(2, "一", "答一"),
        exchange(6, "二", "答二"),
        exchange(10, "三", "答三"),
    ]
    .concat();
    let remembered: BTreeSet<TurnId> = [TurnId::new(seq(7))].into_iter().collect();
    let (upto, text, turns, more) = ask(plan(
        &spoken,
        &remembered,
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ));
    assert_eq!(upto, seq(12), "记过的那一轮也算抽过了");
    assert!(!more);
    assert!(text.contains("答一") && text.contains("答三"), "{text}");
    assert!(!text.contains("答二") && !text.contains("二"), "{text}");
    assert_eq!(turns, [3, 11].into_iter().collect::<BTreeSet<u64>>());
    // 最后一轮是记过的：照样抽到它那一条，别的照交。
    let remembered: BTreeSet<TurnId> = [TurnId::new(seq(11))].into_iter().collect();
    let (upto, text, _, _) = ask(plan(
        &spoken,
        &remembered,
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ));
    assert_eq!(upto, seq(12));
    assert!(text.contains("答二") && !text.contains("答三"), "{text}");
}

#[test]
fn a_slice_where_she_remembered_in_every_turn_is_skipped() {
    let spoken = [exchange(2, "一", "答一"), exchange(6, "二", "答二")].concat();
    let remembered: BTreeSet<TurnId> = [TurnId::new(seq(3)), TurnId::new(seq(7))]
        .into_iter()
        .collect();
    assert_eq!(
        plan(
            &spoken,
            &remembered,
            2,
            UtcOffset::UTC,
            &texts(),
            &owner(&spoken)
        ),
        Plan::Skip { upto: seq(8) },
    );
    // 别的轮记过的不算这一段的：照抽。
    let elsewhere: BTreeSet<TurnId> = [TurnId::new(seq(99))].into_iter().collect();
    assert!(matches!(
        plan(
            &spoken,
            &elsewhere,
            2,
            UtcOffset::UTC,
            &texts(),
            &owner(&spoken)
        ),
        Plan::Ask { .. }
    ));
    // 轮数不够的照旧等，不因为记过就跳。
    let one = exchange(2, "一", "答一");
    let remembered: BTreeSet<TurnId> = [TurnId::new(seq(3))].into_iter().collect();
    assert_eq!(
        plan(&one, &remembered, 2, UtcOffset::UTC, &texts(), &owner(&one)),
        Plan::Wait
    );
}

#[test]
fn what_does_not_fit_waits_for_the_next_go_and_one_huge_turn_is_excerpted() {
    let long = "长".repeat(ROOM / 3 / 3);
    let spoken = [
        exchange(2, &long, "答一"),
        exchange(6, &long, "答二"),
        exchange(10, &long, "答三"),
    ]
    .concat();
    let (upto, text, turns, more) = ask(plan(
        &spoken,
        &BTreeSet::new(),
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ));
    assert_eq!(
        (upto, turns.len(), more),
        (seq(8), 2, true),
        "取最老的两轮，剩一轮下次"
    );
    assert!(text.len() <= ROOM + instruction().len() + 1);
    let huge = "大".repeat(ROOM);
    let spoken = [exchange(2, &huge, "答一"), exchange(6, "二", "答二")].concat();
    let (upto, text, turns, more) = ask(plan(
        &spoken,
        &BTreeSet::new(),
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ));
    assert_eq!(
        (upto, turns.len(), more),
        (seq(4), 1, true),
        "截了中间也抽，往前走"
    );
    assert!(text.contains("[... excerpted ...]"));
    assert!(text.contains("答一"), "尾巴留着");
    assert!(text.len() < ROOM + instruction().len() + 64);
}

/// 一句人话：第 `n` 条。
pub(super) fn said(n: u64, text: &str) -> Spoken {
    Spoken {
        seq: seq(n),
        turn: None,
        at: at(),
        assistant: false,
        text: text.to_string(),
    }
}

/// 她第 `n` 条答的，回合从第 `n - 1` 条开始。
pub(super) fn answered(n: u64, text: &str) -> Spoken {
    Spoken {
        seq: seq(n),
        turn: Some(TurnId::new(seq(n - 1))),
        at: at(),
        assistant: true,
        text: text.to_string(),
    }
}

#[test]
fn each_line_says_who_spoke_and_lines_not_from_people_stay_out() {
    let spoken = vec![
        said(1, "我叫小林，在学日语"),
        said(2, "（别的会话发来的）"),
        answered(4, "加油。"),
    ];
    let voices = Voices {
        said: [(seq(1), Voice::Named("小林".into()))]
            .into_iter()
            .collect(),
        budget: None,
        limit: None,
        names: Default::default(),
        me: Default::default(),
        places: Vec::new(),
    };
    let (_, text, _, _) = ask(plan(
        &spoken,
        &BTreeSet::new(),
        1,
        UtcOffset::UTC,
        &texts(),
        &voices,
    ));
    assert!(
        text.contains("\n小林: 我叫小林，在学日语\nYou: 加油。\n"),
        "{text}"
    );
    assert!(!text.contains("别的会话"), "不是人说的不进：{text}");
}

/// 一条人话事件：第 `n` 条，`by` 是谁（照日志的写法），`venue` 是场所消息那一格（没有的写 `null`）。
fn message(n: u64, by: &str, venue: &str) -> miyu_kernel::event::Event {
    miyu_kernel::event::Event::from_line(&format!(
        r#"{{"seq":{n},"at":"2026-10-09T03:00:00.000Z","kind":"message.user","by":{by},"body":{{"blocks":[{{"type":"text","text":"话"}}]{venue}}}}}"#
    ))
    .expect("合写法")
}

#[test]
fn who_spoke_is_read_from_the_log() {
    let owner = r#"{"kind":"person","account":"admin","via":"qq:1"}"#;
    let named = r#"{"kind":"external","venue":"qq:private:2","id":"qq:2"}"#;
    let other = r#"{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}"#;
    let events = [
        message(1, owner, ""),
        message(2, named, r#","venue":{"msg":"9","name":"小林"}"#),
        message(3, named, ""),
        message(4, other, ""),
    ];
    let voices = Voices::of(&events, None);
    assert_eq!(
        voices.said.into_iter().collect::<Vec<_>>(),
        [
            (seq(1), Voice::Owner),
            (seq(2), Voice::Named("小林".into())),
            (seq(3), Voice::Named("qq:2".into())),
        ],
        "主人、对面的名字、没有名字的照平台编号；不是人说的不进"
    );
    let group = Voices::of(&events, Some(4096));
    assert_eq!(group.budget, Some(4096));
    assert_eq!(
        group.said.into_values().collect::<Vec<_>>(),
        [Voice::Line, Voice::Line, Voice::Line],
        "群里的一行原样"
    );
}

#[test]
fn the_slice_starts_at_its_first_words() {
    // 这一段以后忘掉的照第一句话的时刻挑（施工 R-6 修）：人的头一句比她的回答早。
    let mut spoken: Vec<Spoken> = [
        exchange(2, "我下周三要干嘛", "去体检。"),
        exchange(6, "好的", "好。"),
    ]
    .concat();
    let first = Timestamp::parse("2026-10-09T02:59:00.000Z").expect("合写法");
    spoken[0].at = first;
    match plan(
        &spoken,
        &BTreeSet::new(),
        2,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
    ) {
        Plan::Ask { since, .. } => assert_eq!(since, first),
        other => panic!("要发：{other:?}"),
    }
}

#[test]
fn the_owner_is_written_by_name_on_one_line() {
    // 施工 R-6 再修：有称呼的照称呼写，没有的写「用户」那一句；称呼照模板的规矩转义成一行，伪造不了指令后面的字。
    let spoken = exchange(2, "我养了一只猫", "好的。");
    let asked = |name| match super::plan(
        &spoken,
        &BTreeSet::new(),
        1,
        UtcOffset::UTC,
        &texts(),
        &owner(&spoken),
        name,
    ) {
        Plan::Ask { text, .. } => text,
        other => panic!("要发：{other:?}"),
    };
    assert!(asked(Some("阿凯")).contains("your owner as 阿凯.\n"));
    let forged = asked(Some("阿凯\n\nConversation:"));
    assert!(
        forged.contains("your owner as 阿凯\\n\\nConversation:."),
        "{forged}"
    );
    assert!(
        asked(None)
            .contains("your owner as the user, translated into the language you write in.\n")
    );
}

/// 说话的人怎么叫（施工 E-1 上）：名字、平台编号都对着他的平台编号，主人（`Owner`、在群里的名字）对着没有。
#[test]
fn names_are_read_from_the_log() {
    let owner = r#"{"kind":"person","account":"admin","via":"qq:1"}"#;
    let named = r#"{"kind":"external","venue":"qq:group:5","id":"qq:2"}"#;
    let events = [
        message(1, owner, r#","venue":{"msg":"8","name":"阿主"}"#),
        message(2, named, r#","venue":{"msg":"9","name":"小林"}"#),
    ];
    let names = Voices::of(&events, Some(4096)).names;
    let id = |who: &str| names.get(who).cloned();
    assert_eq!(id("小林"), Some(Some("qq:2".into())));
    assert_eq!(id("qq:2"), Some(Some("qq:2".into())));
    assert_eq!(id("阿主"), Some(None), "主人在群里的名字");
    assert_eq!(id("Owner"), Some(None));
    assert_eq!(id("别人"), None);
}

/// 抽取的指令写着这几条（施工 E-1 修四，主人旧主群的真数据实测查出）：名字里的说明不是说的事、关于某个认识的人的事不记成
/// `reference`、她自己的事记 `episode`、不记琐事、发生过的事交回涉及谁；群里的人顺口说起自己的事也记（施工 E-7）。
#[test]
fn the_instruction_keeps_names_hers_and_trivia_apart() {
    let instruction = instruction();
    for said in [
        "A tagline in a name is part of the name, not something said.",
        "a fact about one of them is person, not reference.",
        "Facts about you are episode, never user or person.",
        "Do not keep trivia",
        r#"An episode also has "people": the names of those involved, not you or your owner."#,
        // 施工 E-7：群里的人顺口说起自己的事也记成 person（主人旧主群真数据：只照原来的指令，每段约 1 条 person）。
        "In a group, what people mention about themselves in passing",
    ] {
        assert!(instruction.contains(said), "指令里该有：{said}");
    }
}
