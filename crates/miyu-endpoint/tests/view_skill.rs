//! 视图里读技能的那一步（施工 9-8 四补，网页写「使用技能 · 写技能」）：真核心的会话用真的 `read` 读家目录里技能的
//! `SKILL.md`，`view.page` 要条目的那一步带 `skill`：名字照效果，给人看的名字照连接的语言挑（`display_name_zh`）。读的那一刻
//! 把显示名记进效果（施工 9-8 五补）：技能删掉以后再翻，照旧是记下的名字。

use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

#[tokio::test]
async fn the_step_that_read_a_skill_carries_its_name_for_people() {
    let home = Home::new();
    home.write(
        "skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\nmetadata:\n  display_name: Notes\n  display_name_zh: 笔记\n---\n照这样记。\n",
    );
    let file = std::fs::canonicalize(home.root.path().join("skills/notes/SKILL.md")).expect("在");
    let args = json!({"file_path": file}).to_string();
    let script = Script::new([Play::calls(&[("read", &args)]), Play::Says("读了。")]);
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let mut client = Client::connect(Arc::new(home.core_full(&script, tools, None, TOKEN)));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    client.say("s1", &session, "记一下").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call("p1", "view.page", json!({"session": session, "view": true}))
        .await;
    let tool = reply["result"]["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["kind"] == "tool"))
        .cloned()
        .unwrap_or_else(|| panic!("有读的那一步：{reply}"));
    assert_eq!(
        tool["skill"],
        json!({"name": "notes", "title": "笔记"}),
        "{tool}"
    );
    // 记进了效果；技能没了，再翻照旧。
    let effects = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            miyu_kernel::event::Body::ToolResult(result) => Some(result.effects),
            _ => None,
        })
        .expect("有结果");
    let recorded = serde_json::to_value(&effects).expect("写得成");
    assert_eq!(
        recorded[0]["display_name"],
        json!({"en": "Notes", "zh": "笔记"}),
        "{recorded}"
    );
    std::fs::remove_dir_all(home.root.path().join("skills/notes")).expect("删得掉");
    let again = client
        .call("p2", "view.page", json!({"session": session, "view": true}))
        .await;
    let tool = again["result"]["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["kind"] == "tool"))
        .cloned()
        .unwrap_or_else(|| panic!("有读的那一步：{again}"));
    assert_eq!(
        tool["skill"],
        json!({"name": "notes", "title": "笔记"}),
        "技能删了照旧：{tool}"
    );
}
