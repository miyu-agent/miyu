//! 人生循环醒来那段话（施工 LF-2，设计 `70-推演人生.md` 第三节第 3 条，`schedules.md` 第三节第 4 条）：真核心、手拨的钟。
//! 交进会话的正文是推演方法、空一行、她上次的留言；推演方法照会话的人格的 `prompts/life.md`，没有、空的、不带人格的用出厂的
//! 那一段，到点那一刻读。她这一轮调 `schedule_wakeup` 没带 `prompt` 的，留言清成空的；她自己定节奏的循环照旧不动。

use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时（`schedules.md` 第三节第 2 条）。
const GAP: i64 = 3_600_000;

/// 出厂的推演方法，去掉末尾的空白。
fn shipped() -> &'static str {
    include_str!("../../../resources/software/life/method.txt").trim_end()
}

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在时刻的范围里")
}

/// 跑在 `session` 上、种类是 `looping` 的一行，留言是 `note`，第一次在 `first`。
fn looping(session: &str, looping: Loop, note: &str, first: i64) -> Wanted {
    Wanted {
        by: Creator::Person,
        when: When::At { at: at(first) },
        prompt: note.to_string(),
        target: Target::Session {
            session: SessionId::parse(session).expect("会话编号合写法"),
        },
        looping: Some(looping),
        first: Some(at(first)),
    }
}

/// 循环那个包的工具：`schedule_wakeup`。
fn catalog() -> Catalog {
    let resources = default_resources();
    Catalog::in_packages([(
        miyu_schedule::LOOP,
        miyu_schedule::loop_tools(&resources).expect("出厂的字"),
    )])
    .expect("合写法")
}

/// 起好唤醒表的核心、开好的会话：`persona` 写了的照它开，没写的不带人格。
async fn started(
    home: &Home,
    script: &Script,
    hand: &Arc<Hand>,
    persona: Option<&str>,
) -> (Arc<Core>, String) {
    let core = core_with_tools(home, script, hand, catalog());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = match persona {
        Some(persona) => client.create_as("c1", &cwd, persona).await,
        None => client.create("c1", &cwd).await,
    };
    core.start_schedules();
    (core, session)
}

/// 等到编号是 `id` 的那一个记了上一次（最多五秒），交回它。
async fn until_recorded(core: &Core, id: &str) -> Schedule {
    let waited = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let schedule = core.schedules().get(id).expect("有这一个");
            if schedule.last.is_some() {
                return schedule;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| panic!("{id} 没跑"))
}

/// 日志里定时交进来的话的正文，照先后。
fn woken(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| {
            let (By::Schedule(_), Body::MessageUser(said)) = (&event.by, &event.body) else {
                return None;
            };
            let text = said
                .blocks
                .iter()
                .map(|block| {
                    let Block::Text(text) = block else {
                        panic!("只有字：{block:?}");
                    };
                    text.text.as_str()
                })
                .collect::<String>();
            Some(text)
        })
        .collect()
}

#[tokio::test]
async fn her_own_method_comes_first_and_an_edit_shows_on_the_next_wakeup() {
    let home = Home::new();
    home.write("root/personas/engineer/prompts/life.md", "照我的过法。\n\n");
    let script = Script::new([Play::Says("醒了。"), Play::Says("又醒了。")]);
    let hand = Hand::at(START);
    let (core, session) = started(&home, &script, &hand, Some("engineer")).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, "昨晚没睡好", START))
        .unwrap();
    home.until_turns(&session, 1).await;
    until_recorded(&core, &added.id).await;
    assert_eq!(woken(&home.log(&session)), ["照我的过法。\n\n昨晚没睡好"]);

    home.write("root/personas/engineer/prompts/life.md", "换个过法。\n");
    hand.advance(GAP);
    home.until_turns(&session, 2).await;
    let said = woken(&home.log(&session));
    assert_eq!(said.len(), 2, "{said:?}");
    assert_eq!(said[1], "换个过法。\n\n昨晚没睡好", "改了下一次就用新的");
}

#[tokio::test]
async fn without_a_method_of_her_own_the_shipped_one_is_used() {
    for (persona, file) in [
        (Some("engineer"), None),
        (Some("engineer"), Some(" \n\n")),
        (None, None),
    ] {
        let home = Home::new();
        if let Some(text) = file {
            home.write("root/personas/engineer/prompts/life.md", text);
        }
        let script = Script::new([Play::Says("醒了。")]);
        let hand = Hand::at(START);
        let (core, session) = started(&home, &script, &hand, persona).await;
        let added = core
            .schedules()
            .add(looping(&session, Loop::Life, "去散步", START))
            .unwrap();
        home.until_turns(&session, 1).await;
        until_recorded(&core, &added.id).await;
        assert_eq!(
            woken(&home.log(&session)),
            [format!("{}\n\n去散步", shipped())],
            "人格 {persona:?}、文件 {file:?}"
        );
    }
}

#[tokio::test]
async fn an_empty_note_leaves_only_the_method() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, session) = started(&home, &script, &hand, Some("engineer")).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, "", START))
        .expect("人生那一行的留言可以是空的");
    home.until_turns(&session, 1).await;
    until_recorded(&core, &added.id).await;
    let said = woken(&home.log(&session));
    assert_eq!(said, [shipped()]);
    assert!(!said[0].ends_with('\n'), "末尾没有多出来的空行");
}

#[tokio::test]
async fn a_note_is_kept_only_when_she_writes_one() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[(
            "schedule_wakeup",
            r#"{"delay_seconds":600,"prompt":"去散步","reason":"在看书"}"#,
        )]),
        Play::Says("十分钟后去散步。"),
        Play::calls(&[("schedule_wakeup", r#"{"delay_seconds":600}"#)]),
        Play::Says("回来了。"),
        Play::Says("又醒了。"),
    ]);
    let hand = Hand::at(START);
    let (core, session) = started(&home, &script, &hand, Some("engineer")).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, "起床", START))
        .unwrap();
    home.until_turns(&session, 1).await;
    until_recorded(&core, &added.id).await;
    assert_eq!(core.schedules().get(&added.id).unwrap().prompt, "去散步");

    hand.advance(600_000);
    home.until_turns(&session, 2).await;
    assert_eq!(
        core.schedules().get(&added.id).unwrap().prompt,
        "",
        "这一轮没写的清成空的"
    );

    hand.advance(600_000);
    home.until_turns(&session, 3).await;
    assert_eq!(
        woken(&home.log(&session)),
        [
            format!("{}\n\n起床", shipped()),
            format!("{}\n\n去散步", shipped()),
            shipped().to_string(),
        ],
        "旧的不再念一遍"
    );
}

#[tokio::test]
async fn a_dynamic_loop_keeps_its_prompt_and_its_words() {
    let home = Home::new();
    home.write("root/personas/engineer/prompts/life.md", "照我的过法。\n");
    let script = Script::new([
        Play::calls(&[("schedule_wakeup", r#"{"delay_seconds":600}"#)]),
        Play::Says("十分钟后再看。"),
    ]);
    let hand = Hand::at(START);
    let (core, session) = started(&home, &script, &hand, Some("engineer")).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Dynamic, "盯 CI", START))
        .unwrap();
    home.until_turns(&session, 1).await;
    until_recorded(&core, &added.id).await;
    let row = core.schedules().get(&added.id).unwrap();
    assert_eq!(row.prompt, "盯 CI", "没写的不动");
    assert_eq!(row.next, Some(at(START + 600_000)));
    assert_eq!(
        woken(&home.log(&session)),
        ["盯 CI"],
        "原话原样，不拼推演方法"
    );
}
