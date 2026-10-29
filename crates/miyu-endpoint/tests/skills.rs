//! 技能（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 2、3 条）：真核心走一遍。数据根里放的技能列进 system，她读得
//! 到 `SKILL.md`；换一份核心像重启过，载入的会话照样读得到（文件夹放开只读是造会话、载入时各找一次）；沙盒里的命令的规格
//! 两次都单独放开技能文件夹（施工 X-10 中）。

use std::sync::Arc;

use serde_json::json;

use crate::support::{Client, Home, TOKEN};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

#[tokio::test]
async fn skills_in_the_home_are_listed_and_read_before_and_after_a_restart() {
    let home = Home::new();
    home.write(
        "skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\n---\n\n照这样记。\n",
    );
    let skill = home
        .root
        .path()
        .join("skills")
        .join("notes")
        .join("SKILL.md")
        .to_string_lossy()
        .into_owned();
    // 她照列出来的那条读：Windows 上反斜杠换成了 `/`。
    let skill = if cfg!(windows) {
        skill.replace('\\', "/")
    } else {
        skill
    };
    let folder = std::fs::canonicalize(home.root.path().join("skills").join("notes")).expect("在");
    let read = Fake::new("read", Access::Read, Act::Echo);
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = || {
        Catalog::new([
            Arc::clone(&read) as Arc<dyn Tool>,
            Arc::clone(&run) as Arc<dyn Tool>,
        ])
        .expect("合写法")
    };
    let script = || {
        Script::new([
            Play::calls(&[("read", &json!({ "path": skill }).to_string())]),
            Play::calls(&[("run", "{}")]),
            Play::Says("好。"),
        ])
    };
    let work = home.work.to_string_lossy().into_owned();
    let first = script();
    let mut client = Client::connect(home.core_with_tools(&first, tools(), TOKEN));
    client.hello_without_input().await;
    let session = client.create("c1", &work).await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    assert_eq!(read.calls().len(), 1, "造的会话读得到技能");
    let system = &first.requests()[0].1.system;
    // 出厂包带的技能也列着（施工 X-13 下），照名字排：家目录里的这一个在前面。
    assert!(
        system.contains(&format!("- notes: Take notes. ({skill})\n")),
        "{system}"
    );
    // 换一份核心，像重启过：会话从磁盘载入。
    let mut client = Client::connect(home.core_with_tools(&script(), tools(), TOKEN));
    client.hello_without_input().await;
    client.say("c3", &session, "again").await;
    home.until_turns(&session, 2).await;
    assert_eq!(read.calls().len(), 2, "载入的会话照样读得到");
    // 沙盒里的命令也读得到（施工 X-10 中）：造的、载入的会话，规格都单独放开技能文件夹。
    let reveals: Vec<Vec<std::path::PathBuf>> = run
        .calls()
        .into_iter()
        .map(|call| call.sandbox.expect("带着沙盒").spec.reveal.clone())
        .collect();
    // 出厂包带的技能文件夹也在里面（施工 X-13 下）。
    assert_eq!(reveals.len(), 2);
    assert!(
        reveals.iter().all(|one| one.contains(&folder)),
        "{reveals:?}"
    );
}

/// 请求里说的话：每一条消息里的字接起来（施工 PT-2 外：看尾巴上交的技能列表）。
fn words(request: &miyu_kernel::request::Request) -> String {
    use miyu_kernel::block::Block;
    use miyu_kernel::request::Message;
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks }
            | Message::Assistant { blocks }
            | Message::Tool { blocks, .. } => blocks.iter(),
        })
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

/// 别的技能目录（施工 PT-2 外）：关着的（出厂）一个都不碰，家目录下放着 `~/.agents/skills` 的，造的会话发出去的 system、工具和
/// 没放时逐字节一样（消息里有会话编号，两个会话本来就不一样）；会话中途开了的，下一轮照技能本来的规矩在尾巴上交一份新列表，system、工具都不动；开着造的会话列进 system。
#[tokio::test]
async fn other_skill_folders_turned_off_leave_the_request_as_it_was() {
    let home = Home::new();
    let read = Fake::new("read", Access::Read, Act::Echo);
    let tools = || Catalog::new([Arc::clone(&read) as Arc<dyn Tool>]).expect("合写法");
    let work = home.work.to_string_lossy().into_owned();
    let listed = "- elsewhere: Shared.";

    let before = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core_elsewhere(&before, tools()));
    client.hello_without_input().await;
    let session = client.create("b1", &work).await;
    client.say("b2", &session, "hi").await;
    home.until_turns(&session, 1).await;

    home.write_elsewhere(".agents/skills", "elsewhere", "Shared.");
    let off = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(home.core_elsewhere(&off, tools()));
    client.hello_without_input().await;
    let session = client.create("o1", &work).await;
    client.say("o2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (before, first) = (&before.requests()[0].1, &off.requests()[0].1);
    assert_eq!(first.system, before.system, "关着的 system 逐字节一样");
    assert_eq!(first.tools, before.tools);
    assert!(!first.system.contains(listed));

    let set = client
        .call(
            "o3",
            "config.set",
            serde_json::json!({"layer": "system", "changes": [
                {"key": "skills.external.enabled", "value": true},
            ]}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    client.say("o4", &session, "again").await;
    home.until_turns(&session, 2).await;
    let second = &off.requests()[1].1;
    assert_eq!(second.system, first.system, "中途开的 system 不动");
    assert_eq!(second.tools, first.tools, "工具不动");
    let said = words(second);
    assert_eq!(said.matches("<skills-changed>").count(), 1, "{said}");
    assert!(said.contains(listed), "{said}");

    let on = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core_elsewhere(&on, tools()));
    client.hello_without_input().await;
    let session = client.create("n1", &work).await;
    client.say("n2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let system = &on.requests()[0].1.system;
    assert!(system.contains(listed), "开着造的列进 system：{system}");
}
