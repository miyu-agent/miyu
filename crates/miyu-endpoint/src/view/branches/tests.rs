//! 支线怎么认、怎么列（施工 VW-2）：从这个会话分出去、不是 fork 子代理的才是；跑完的是 `done`，最后一轮被打断的是 `stopped`，
//! 在跑的、还没跑完第一轮的是 `running`；标题照会话列表的算法；做完的只留最近的 [`KEPT`] 条，在跑的都列。

use super::*;

const PARENT: &str = "01a1e8da-2977-7584-93c4-ac1e904048a6";
const CHILD: &str = "01a1e8db-0000-7000-8000-000000000001";

/// 支线的第一条：从 `from` 分出来，`within` 写了的是 fork 子代理。
fn created(from: &str, within: bool) -> String {
    let within = if within { r#","within":3"# } else { "" };
    format!(
        r#"{{"seq":5,"at":"2026-11-17T09:00:00.000Z","kind":"session.created","by":{{"kind":"person"}},"cause":"c-1","body":{{"venue":"qq:group:777","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{{"level":"full","read_only":false}},"parent":"{PARENT}","forked":{{"from":"{from}","upto":4,"seal_after":1{within}}}}}}}"#
    )
}

fn events(lines: &[String]) -> Vec<Event> {
    lines
        .iter()
        .map(|line| Event::from_line(line).unwrap_or_else(|e| panic!("{line}: {e}")))
        .collect()
}

const SAID: &str = r#"{"seq":6,"at":"2026-11-17T09:00:01.000Z","kind":"message.user","by":{"kind":"external","venue":"qq:group:777","id":"qq:20017","role":"member"},"cause":"c-2","body":{"blocks":[{"type":"text","text":"小林说的那一句\n第二行"}],"venue":{"msg":"8810","name":"小林"}}}"#;

fn ended(reason: &str) -> String {
    format!(
        r#"{{"seq":9,"at":"2026-11-17T09:00:09.000Z","kind":"turn.ended","turn":1,"by":{{"kind":"kernel"}},"body":{{"reason":"{reason}"}}}}"#
    )
}

#[test]
fn only_a_fork_of_this_session_is_a_branch() {
    let (parent, child) = (
        SessionId::parse(PARENT).unwrap(),
        SessionId::parse(CHILD).unwrap(),
    );
    let done = events(&[created(PARENT, false), SAID.to_string(), ended("completed")]);
    let branch = shaped(&parent, &child, &done, false).expect("是支线");
    assert_eq!(branch.title, "小林说的那一句", "照第一句话的头一行");
    assert_eq!((branch.state, branch.ended.is_some()), ("done", true));
    assert_eq!(
        shaped(&parent, &child, &done, true).unwrap().state,
        "running",
        "在跑一轮"
    );
    let cut = events(&[
        created(PARENT, false),
        SAID.to_string(),
        ended("interrupted"),
    ]);
    assert_eq!(
        shaped(&parent, &child, &cut, false).unwrap().state,
        "stopped"
    );
    let waiting = events(&[created(PARENT, false)]);
    assert_eq!(
        shaped(&parent, &child, &waiting, false).unwrap().state,
        "running",
        "第一轮还没跑完"
    );
    let other = events(&[created(CHILD, false)]);
    assert!(
        shaped(&parent, &child, &other, false).is_none(),
        "别的会话分出来的"
    );
    let agent = events(&[created(PARENT, true)]);
    assert!(
        shaped(&parent, &child, &agent, false).is_none(),
        "fork 子代理不是支线"
    );
}

#[test]
fn finished_branches_keep_the_newest_and_running_ones_all_stay() {
    let mut branches = Branches::default();
    let base = Timestamp::parse("2026-11-17T09:00:00.000Z").unwrap();
    for n in 0..46u32 {
        let id = SessionId::parse(&format!("01a1e8db-0000-7000-8000-{n:012}")).unwrap();
        let at = Timestamp::from_unix_millis(base.unix_millis() + i64::from(n) * 60_000).unwrap();
        let running = n == 0 || n == 45;
        branches.seen.insert(
            id,
            Some(Branch {
                title: format!("支线 {n}"),
                started: at,
                ended: (!running).then_some(at),
                state: if running { "running" } else { "done" },
            }),
        );
    }
    let rows = branches.rows();
    assert_eq!(rows.len(), KEPT + 2, "做完的留 {KEPT} 条，在跑的两条都在");
    assert_eq!(rows[0]["title"], "支线 0", "最早的那条还在跑，照样列");
    assert_eq!(
        rows[1]["title"], "支线 25",
        "做完的 1～44 只留最近的 25～44"
    );
    assert_eq!(rows.last().unwrap()["title"], "支线 45");
    let bytes = serde_json::to_string(&rows).unwrap().len();
    assert!(
        bytes < 8 * 1024,
        "一天四十几条支线，列出来的不到 8 KB：{bytes}"
    );
}
