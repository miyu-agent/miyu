//! 技能（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 2、3 条）：真核心走一遍。属主家目录里放的技能列进 system，她读得
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
