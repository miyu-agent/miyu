//! 出厂的预设「推演人生」（施工 LF-5 中，LF-4 下起多 `end_life`，`docs/blueprint/presets.md`「出厂的几份」，设计 `70-推演人生.md` 第三节第 7 条）：
//! `preset.list` 列它、`hidden` 是 `true`；`life.start` 开的会话照它，工具面里有搜索、取网页、记忆三件、`schedule_wakeup`、
//! `history`，没有命令行、读写文件、定时任务。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::preset_face::tool_names;
use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 基础系统的工具（一件一个包）、联网两件、记忆三件、循环一件、结束人生一件（施工 LF-4 下）、定时任务一件：都照包登记。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    let basesystem = [
        "read",
        "write",
        "edit",
        "glob",
        "grep",
        "trash",
        "shell",
        "jobs",
        "subagent",
        "sessions",
        "ask_user",
        "todowrite",
        "history",
        "show",
    ];
    let mut groups = miyu_basesystem::grouped(basesystem.map(fake).to_vec());
    groups.push(("base-net", vec![fake("web_search"), fake("web_fetch")]));
    groups.push((
        "base-memory",
        vec![fake("remember"), fake("memory_search"), fake("forget")],
    ));
    groups.push((miyu_schedule::LOOP, vec![fake("schedule_wakeup")]));
    groups.push((miyu_schedule::LIFE, vec![fake("end_life")]));
    groups.push((miyu_schedule::SCHEDULE, vec![fake("cron_create")]));
    Catalog::in_packages(groups).expect("合写法")
}

#[tokio::test]
async fn the_life_preset_is_listed_hidden_and_gives_only_its_tools() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, catalog());
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let listed = client.call("p", "preset.list", json!({})).await;
    let life = listed["result"]["presets"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|one| one["preset"] == "life")
        .unwrap_or_else(|| panic!("列了推演人生：{listed}"))
        .clone();
    assert_eq!(life["hidden"], json!(true), "{life}");
    assert_eq!(life["icon"], json!("sprout"), "{life}");
    let got = client
        .call("g", "preset.get", json!({"preset": "life"}))
        .await;
    assert_eq!(
        got["result"]["unattended_steps"],
        json!(null),
        "不另设步数上限：{got}"
    );

    let reply = client
        .call("l", "life.start", json!({"persona": "engineer"}))
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("开出来了：{reply}"))
        .to_string();
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    let mut names = tool_names(&request);
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "end_life",
            "forget",
            "history",
            "memory_search",
            "remember",
            "schedule_wakeup",
            "web_fetch",
            "web_search"
        ]
    );
}
