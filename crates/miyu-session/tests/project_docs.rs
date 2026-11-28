//! 项目指令（施工 P-14，`internal/blueprint/project-docs.md`）：本机的会话第一轮把仓库里的 AGENTS.md 交给她；没变的下一轮不重交；
//! 改了的换一份；场所会话不读。施工 P-14 下：第一次读子目录里的文件时，那几层的接在结果后面，读同一个子目录的不重交。

use std::path::PathBuf;

use miyu_kernel::block::Block;
use miyu_kernel::id::VenueId;
use miyu_kernel::request::{Message, Request};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 家目录里一个仓库，根上一份 AGENTS.md：交回仓库的真实路径。
fn repo(home: &Home, text: &str) -> PathBuf {
    let repo = home.home.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::write(repo.join("AGENTS.md"), text).expect("写得进");
    repo.canonicalize().expect("在")
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

async fn session(home: &Home, script: &Script, cwd: &std::path::Path, venue: &str) -> Handle {
    session_with(home, script, cwd, venue, Catalog::default()).await
}

async fn session_with(
    home: &Home,
    script: &Script,
    cwd: &std::path::Path,
    venue: &str,
    tools: Catalog,
) -> Handle {
    let opening = Opening {
        cwd: cwd.to_string_lossy().into_owned(),
        ..Opening::default()
    };
    let lines = Lines {
        venue: VenueId::parse(venue).expect("合写法"),
        ..Lines::default()
    };
    home.create_full(script, &tools, opening, lines).await
}

#[tokio::test]
async fn told_on_the_first_turn_once_and_again_when_changed() {
    let home = Home::new();
    let repo = repo(&home, "Run the tests before you say done.\n");
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let handle = session(&home, &script, &repo, "local").await;
    turn(&handle, "cmd-1").await;
    turn(&handle, "cmd-2").await;
    std::fs::write(repo.join("AGENTS.md"), "Use tabs.\n").expect("写得进");
    turn(&handle, "cmd-3").await;
    let requests = script.requests();
    let first = words(&requests[0].1);
    assert!(
        first.contains("<project-instructions path=")
            && first.contains("Run the tests before you say done."),
        "{first}"
    );
    let second = words(&requests[1].1);
    assert_eq!(
        second.matches("<project-instructions").count(),
        1,
        "没变的不重交：{second}"
    );
    let third = words(&requests[2].1);
    assert!(
        third.contains("replace the earlier ones") && third.contains("Use tabs."),
        "{third}"
    );
}

#[tokio::test]
async fn a_venue_session_does_not_read_them() {
    let home = Home::new();
    let repo = repo(&home, "Secret house rules.\n");
    let script = Script::new([Play::Says("好。")]);
    let handle = session(&home, &script, &repo, "qq:group:1").await;
    turn(&handle, "cmd-1").await;
    let requests = script.requests();
    assert!(
        !words(&requests[0].1).contains("Secret house rules."),
        "场所会话不读"
    );
}

/// 仓库里两层子目录各一份：读最里面的一个文件，两份都接在那次结果后面；再读同一个子目录的另一个文件不重交；根上那份回合开始
/// 交过，不跟着读重交。
fn nested_repo(home: &Home) -> PathBuf {
    let repo = repo(home, "Root rules.\n");
    std::fs::create_dir_all(repo.join("sub/deep")).expect("建得了");
    std::fs::write(repo.join("sub/AGENTS.md"), "Sub rules.\n").expect("写得进");
    std::fs::write(repo.join("sub/deep/CLAUDE.md"), "Deep rules.\n").expect("写得进");
    for name in ["a.txt", "b.txt"] {
        std::fs::write(repo.join("sub/deep").join(name), "x\n").expect("写得进");
    }
    repo
}

/// 真的基础系统：要 `read`。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 读 `path` 的一次调用的参数。
fn read(path: &str) -> String {
    serde_json::json!({ "file_path": path }).to_string()
}

#[tokio::test]
async fn a_subfolder_s_instructions_come_with_the_first_read_inside_it() {
    let home = Home::new();
    let repo = nested_repo(&home);
    let script = Script::new([
        Play::calls(&[("read", &read("sub/deep/a.txt"))]),
        Play::calls(&[("read", &read("sub/deep/b.txt"))]),
        Play::Says("好。"),
    ]);
    let handle = session_with(&home, &script, &repo, "local", basesystem(&home)).await;
    turn(&handle, "cmd-1").await;
    let requests = script.requests();
    let after_first = words(&requests[1].1);
    assert!(
        after_first.contains("Sub rules.") && after_first.contains("Deep rules."),
        "两层都接在结果后面：{after_first}"
    );
    assert!(
        after_first.find("Sub rules.") < after_first.find("Deep rules."),
        "从外到里"
    );
    let after_second = words(&requests[2].1);
    for rule in ["Root rules.", "Sub rules.", "Deep rules."] {
        assert_eq!(
            after_second.matches(rule).count(),
            1,
            "{rule} 只交一次：{after_second}"
        );
    }
}

#[tokio::test]
async fn a_venue_session_does_not_add_a_subfolder_s_instructions() {
    let home = Home::new();
    let repo = nested_repo(&home);
    let script = Script::new([
        Play::calls(&[("read", &read("sub/deep/a.txt"))]),
        Play::Says("好。"),
    ]);
    let handle = session_with(&home, &script, &repo, "qq:group:1", basesystem(&home)).await;
    turn(&handle, "cmd-1").await;
    let requests = script.requests();
    assert!(
        !words(&requests[1].1).contains("Sub rules."),
        "场所会话不读"
    );
}

/// 请求形状探针（施工 AUD-7）：两层分一个上限、上限处切开一个字、`CLAUDE.md` 是指向普通文件的链接、子目录那份比上限长，
/// 整份请求写成 JSON 交到 `MIYU_DOCS_SHAPE_OUT`（家目录换成 `<HOME>`）。改读法前后各跑一次、逐字节比。
#[tokio::test]
#[ignore = "探针：手动跑，比改前改后的请求字节"]
async fn request_shape_probe_project_docs() {
    let Some(out) = std::env::var_os("MIYU_DOCS_SHAPE_OUT") else {
        return;
    };
    let home = Home::new();
    let repo = repo(&home, &format!("{}\n", "R".repeat(20_000)));
    std::fs::create_dir_all(repo.join("sub/deep")).expect("建得了");
    let rules = format!("{}{}tail\n", "s".repeat(12_765), "😀".repeat(2));
    std::fs::write(repo.join("rules.md"), rules).expect("写得进");
    #[cfg(unix)]
    std::os::unix::fs::symlink(repo.join("rules.md"), repo.join("sub/CLAUDE.md")).expect("建得了");
    let deep = format!("{}{}\n", "d".repeat(32_767), "é".repeat(4000));
    std::fs::write(repo.join("sub/deep/AGENTS.md"), deep).expect("写得进");
    std::fs::write(repo.join("sub/deep/a.txt"), "x\n").expect("写得进");
    let cwd = repo.join("sub");
    let script = Script::new([
        Play::calls(&[("read", &read("deep/a.txt"))]),
        Play::Says("好。"),
    ]);
    let handle = session_with(&home, &script, &cwd, "local", basesystem(&home)).await;
    turn(&handle, "cmd-1").await;
    let requests: Vec<Request> = script.requests().into_iter().map(|(_, r)| r).collect();
    let json = serde_json::to_string_pretty(&requests).expect("写得成 JSON");
    let home_text = home
        .home
        .canonicalize()
        .expect("在")
        .to_string_lossy()
        .into_owned();
    std::fs::write(out, json.replace(&home_text, "<HOME>")).expect("写得进");
}
