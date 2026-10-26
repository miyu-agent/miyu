//! 成员不能装包（施工 Y-5 下下）：成员自己的会话里她调 `package`（施工 BS-1 中起经命令里的 `miyu pkg`，`support::pkg`）装带
//! `[process]` 的、只带技能的包，看一眼那一步就拒
//! `not_permitted`、不问人、什么都不装；机主的会话照装。端口照会话的属主判（`packages/port.rs`）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::AccountId;
use miyu_session::testkit::{Play, Script};

use crate::support::extensions::{dir, until_turns_in};
use crate::support::login::member;
use crate::support::*;

/// 在 `dir` 里放一个带进程的包 `id`，交回它的文件夹。
fn extension(dir: &Path, id: &str) -> PathBuf {
    let folder = dir.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-{id}\"\nabout = {{ en = \"X\" }}\n\n[process]\n"
        ),
    )
    .expect("写得进");
    folder
}

/// 账号 `account` 名下会话 `session` 里每一条工具结果的字。
fn results(home: &Home, account: &AccountId, session: &str) -> Vec<String> {
    miyu_store::log::read_events(&dir(home, account, session))
        .expect("读得了")
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

/// 真核心，工具目录是基础系统：她跑一条命令，里面的 `miyu pkg` 照先后调 `commands`（`package` 的参数），然后说一句。
fn started(home: &Home, commands: Vec<serde_json::Value>) -> Arc<miyu_endpoint::Core> {
    let commands: Vec<String> = commands.iter().map(ToString::to_string).collect();
    let commands: Vec<(&str, &str)> = commands
        .iter()
        .map(|args| ("package", args.as_str()))
        .collect();
    let plays = vec![
        Play::calls(&[("shell", &pkg::run(&commands))]),
        Play::Says("好了。"),
    ];
    Arc::new(home.core_full(&Script::new(plays), pkg::catalog(), None, TOKEN))
}

#[tokio::test]
async fn a_member_s_session_cannot_install_a_package_with_a_process() {
    let home = Home::new();
    let bob = AccountId::parse("bob").expect("合写法");
    let workspace = home.root.workspace(&bob);
    // 包放进他自己的工作区：注册以后才建（家目录先在的话用户名就算占了）。
    let folder = workspace.join("ext");
    let core = started(&home, vec![json!({"install": folder})]);
    let mut client = member(&core, "bob").await;
    extension(&workspace, "ext");
    let session = client.create("c1", &workspace.to_string_lossy()).await;
    client.say("s1", &session, "装上它").await;
    until_turns_in(&dir(&home, &bob, &session), 1).await;
    let said = results(&home, &bob, &session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("not_permitted"), "{said:?}");
    assert!(!home.root.path().join("home/bob/packages/ext").exists());
    assert!(!home.root.path().join("system/packages/ext").exists());
}

#[tokio::test]
async fn the_owner_s_session_still_installs_it() {
    let home = Home::new();
    let folder = extension(&home.work, "ext");
    let core = started(&home, vec![json!({"install": folder})]);
    let mut client = Client::connect(core);
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    let params = json!({"session": session, "level": "full"});
    let reply = client
        .call("p1", "session.set_permission_level", params)
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    client.say("s1", &session, "装上它").await;
    home.until_turns(&session, 1).await;
    assert!(
        home.root.path().join("system/packages/ext").exists(),
        "{:?}",
        results(&home, &alice(), &session)
    );
}

/// 不带进程的包成员也装不了（施工 Y-5 下下，主会话 2026-10-24 定）：现在装进的是机主家目录那一层、对所有人生效，等 U23「装进
/// 自己家目录」再放开。
#[tokio::test]
async fn a_member_s_session_cannot_install_a_package_without_a_process_either() {
    let home = Home::new();
    let bob = AccountId::parse("bob").expect("合写法");
    let workspace = home.root.workspace(&bob);
    let folder = workspace.join("kit");
    let core = started(&home, vec![json!({"install": folder})]);
    let mut client = member(&core, "bob").await;
    std::fs::create_dir_all(folder.join("skills/notes")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Kit\" }\n",
    )
    .expect("写得进");
    std::fs::write(
        folder.join("skills/notes/SKILL.md"),
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    )
    .expect("写得进");
    let session = client.create("c1", &workspace.to_string_lossy()).await;
    client.say("s1", &session, "装上它").await;
    until_turns_in(&dir(&home, &bob, &session), 1).await;
    let said = results(&home, &bob, &session);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("not_permitted"), "{said:?}");
    assert!(!home.root.path().join("home/bob/packages/kit").exists());
    assert!(!home.root.path().join("system/packages/kit").exists());
}

/// 成员会话里她调 `package` 卸机主装的包、启用机主装的扩展（施工 Y-5 下下，主会话 2026-10-24 审出的另两半）：都拒
/// `not_permitted`，包还在、扩展没开、能力没批。
#[tokio::test]
async fn a_member_s_session_cannot_remove_or_enable_either() {
    let home = Home::new();
    home.write(
        "system/packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write(
        "system/packages/kit/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    );
    let ext = extension(&home.root.path().join("system/packages"), "ext");
    std::fs::write(
        ext.join("package.toml"),
        "[package]\nprotocol = [1, 1]\nname = { en = \"Ext\" }\n\n[command]\nname = \"ext\"\nprogram = \"miyu-ext\"\nabout = { en = \"X\" }\n\n[process]\ncapabilities = [\"network\"]\n",
    )
    .expect("写得进");
    let bob = AccountId::parse("bob").expect("合写法");
    let workspace = home.root.workspace(&bob);
    let core = started(
        &home,
        vec![json!({"remove": "kit"}), json!({"enable": "ext"})],
    );
    let mut client = member(&core, "bob").await;
    let session = client.create("c1", &workspace.to_string_lossy()).await;
    client.say("s1", &session, "卸了它、开那个").await;
    until_turns_in(&dir(&home, &bob, &session), 1).await;
    let said = results(&home, &bob, &session);
    assert_eq!(said.len(), 1, "一条命令：{said:?}");
    assert_eq!(
        said[0].matches("not_permitted").count(),
        2,
        "卸、开都拒：{said:?}"
    );
    assert!(
        home.root.path().join("system/packages/kit").exists(),
        "包还在"
    );
    let switches =
        std::fs::read_to_string(home.root.system().join("extensions.json")).unwrap_or_default();
    assert!(!switches.contains("ext"), "扩展没开：{switches}");
    let groups =
        std::fs::read_to_string(home.root.system().join("groups.json")).unwrap_or_default();
    assert!(!groups.contains("ext"), "能力没批：{groups}");
}
