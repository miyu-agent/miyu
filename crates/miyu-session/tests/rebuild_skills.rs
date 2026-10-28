//! 用过的技能真的压后重建（施工 X-10 下）：第一轮她读了五个小文件、技能的 `SKILL.md` 和一个大文件，读完就过线，这一轮里
//! 接着压。五个小文件占满文件的名额，技能另算一组，照样从磁盘重读回来，排在文件后面；压完以后的请求里看得到技能的正文。

use std::path::Path;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, ContextCompacted, Level, Permission};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 请求里的全部字，连成一段。
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

#[tokio::test]
async fn a_used_skill_comes_back_after_the_files_fill_their_slots() {
    let home = Home::new();
    let dir = home.root.path().join("skills").join("notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    let body = "---\nname: notes\ndescription: Take notes.\n---\n照这样记。\n";
    std::fs::write(dir.join("SKILL.md"), body).expect("写得进");
    let skill = dir.join("SKILL.md").to_string_lossy().into_owned();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let names = ["a", "b", "c", "d", "e"];
    for name in names {
        std::fs::write(work.join(format!("{name}.txt")), format!("{name}\n")).expect("写得进");
    }
    // 读它就过线（同 `rebuild.rs`）。
    let big: String = (0..1000)
        .map(|n| format!("{n:05} lorem ipsum dolor sit amet\n"))
        .collect();
    std::fs::write(work.join("big.txt"), &big).expect("写得进");
    let args = |path: &str| serde_json::json!({ "file_path": path }).to_string();
    let mut calls: Vec<String> = names
        .iter()
        .map(|name| args(&format!("{name}.txt")))
        .collect();
    calls.push(args(&skill));
    calls.push(args("big.txt"));
    let calls: Vec<(&str, &str)> = calls.iter().map(|call| ("read", call.as_str())).collect();
    let script = Script::new([
        Play::calls(&calls),
        Play::Says("好。"),
        Play::Says("<summary>她读了几个文件和一个技能。</summary>"),
        Play::Says("嗯。"),
    ])
    .window(32_000);
    let opening = Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    };
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let handle = home.create_as(&script, &tools, opening).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("读一下"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("接着来"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let compacted: ContextCompacted = home
        .log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了");
    let restored: Vec<&str> = compacted
        .restored
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(restored.len(), 6, "五个文件加一个技能：{restored:?}");
    assert!(
        restored[..5].iter().all(|path| path.ends_with(".txt")),
        "{restored:?}"
    );
    let last = compacted.restored.last().expect("有");
    assert!(
        last.path.ends_with("SKILL.md"),
        "技能排在文件后面：{restored:?}"
    );
    assert_eq!(last.blob, ContentHash::of(body.as_bytes()));
    let requests = script.requests();
    let after = &requests.last().expect("有请求").1;
    assert!(words(after).contains("照这样记。"), "{}", words(after));
}
