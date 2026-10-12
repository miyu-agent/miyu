//! 会话中途技能列表变了（施工 X-11 下，`docs/blueprint/skills.md`「定了的」第 2 条）：回合开始照这一刻再找一遍，和她最后看到的
//! 不一样的，在这一轮的尾巴上交一份新的完整列表（`<skills-changed>`），system 不改；交过的下一轮不再交；新放的文件夹当场
//! 放开只读；一个都没有了的说一句；什么都没变的不交。

use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 在 alice 家目录里放技能 `name`；交回 `SKILL.md` 的路径（列出来的写法：Windows 上反斜杠换成 `/`）。
fn put(home: &Home, name: &str) -> String {
    let dir = home
        .root
        .account_dir(&alice_account())
        .join("skills")
        .join(name);
    std::fs::create_dir_all(&dir).expect("建得了");
    let file = dir.join("SKILL.md");
    std::fs::write(
        &file,
        format!("---\nname: {name}\ndescription: Do {name}.\n---\n正文\n"),
    )
    .expect("写得进");
    let file = file.to_string_lossy().into_owned();
    if cfg!(windows) {
        file.replace('\\', "/")
    } else {
        file
    }
}

/// 请求里的全部字（不算 system），连成一段。
fn words(request: &Request) -> String {
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

async fn turn(handle: &Handle, command: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn a_skill_added_mid_session_comes_in_the_tail_once_and_opens_its_folder() {
    let home = Home::new();
    put(&home, "notes");
    let read = Fake::new("read", Access::Read, Act::Echo);
    let catalog = Catalog::new([Arc::clone(&read) as Arc<dyn Tool>]).expect("合写法");
    let added = home
        .root
        .account_dir(&alice_account())
        .join("skills")
        .join("pdf")
        .join("SKILL.md")
        .to_string_lossy()
        .into_owned();
    let script = Script::new([
        Play::Says("一。"),
        Play::calls(&[("read", &serde_json::json!({ "path": added }).to_string())]),
        Play::Says("二。"),
        Play::Says("三。"),
    ]);
    let handle = home.create_with(&script, &catalog).await;
    turn(&handle, "cmd-1").await;
    put(&home, "pdf");
    turn(&handle, "cmd-2").await;
    turn(&handle, "cmd-3").await;
    let requests = script.requests();
    assert!(
        !words(&requests[0].1).contains("<skills-changed>"),
        "没变的不交"
    );
    let second = words(&requests[1].1);
    assert_eq!(second.matches("<skills-changed>").count(), 1, "{second}");
    assert!(second.contains("- notes: Do notes."), "完整列表：{second}");
    assert!(second.contains("- pdf: Do pdf."), "{second}");
    assert_eq!(read.calls().len(), 1, "新放的当场放开只读");
    assert_eq!(requests[1].1.system, requests[0].1.system, "system 不改");
    let last = words(&requests.last().expect("有").1);
    assert_eq!(
        last.matches("<skills-changed>").count(),
        1,
        "交过的不再交：{last}"
    );
}

#[tokio::test]
async fn when_every_skill_is_gone_the_tail_says_so() {
    let home = Home::new();
    put(&home, "notes");
    let read = Fake::new("read", Access::Read, Act::Echo);
    let catalog = Catalog::new([Arc::clone(&read) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let handle = home.create_with(&script, &catalog).await;
    turn(&handle, "cmd-1").await;
    std::fs::remove_dir_all(home.root.account_dir(&alice_account()).join("skills"))
        .expect("删得了");
    turn(&handle, "cmd-2").await;
    let second = words(&script.requests()[1].1);
    assert!(
        second.contains("<skills-changed>\nNo skills are available now.\n</skills-changed>\n"),
        "{second}"
    );
}
