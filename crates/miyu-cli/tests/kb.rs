//! `miyu kb`（施工 K-7 下，`docs/blueprint/cli/kb.md`）：在进程里起一个核心，每个子命令走一遍：印什么、退出码；加文件照本机
//! 路径传上去再加进库、`--to` 放进库里的目录、已经有的拒、`--replace` 换；挂目录照当前目录接相对路径；核心拒的照原话说，挂不上的
//! 接上为什么；`edit` 什么都没写的说一句、退出码 1。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;

use crate::support::{Asked, Home, Tape, within};
use miyu_cli::language::Language;
use miyu_cli::{Kb, KbPlan, kb_on};
use miyu_session::testkit::Script;

/// 照主程序的样子读参数：`miyu kb …`。
#[derive(Parser)]
struct Line {
    #[command(subcommand)]
    command: Top,
}

#[derive(clap::Subcommand)]
enum Top {
    Kb(Kb),
}

fn parsed(words: &[&str]) -> Kb {
    let line = std::iter::once("miyu")
        .chain(std::iter::once("kb"))
        .chain(words.iter().copied());
    match Line::try_parse_from(line).expect("参数读得懂").command {
        Top::Kb(kb) => kb,
    }
}

/// 数据根旁边的一个目录：加的文件、挂的目录放这里（数据根里的 `blob.put` 不收）。
fn outside(home: &Home) -> PathBuf {
    let dir = home.root.path().parent().expect("有上一级").join("outside");
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

/// 在真的套接字上连上核心，照 `words`（`kb` 后面的）跑一次；界面是中文，当前目录是 `cwd`。
async fn run(home: &Home, cwd: &Path, words: &[&str]) -> Asked {
    let plan = KbPlan {
        args: parsed(words),
        language: Language::Chinese,
        cwd: cwd.to_path_buf(),
    };
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let code = within("跑完", kb_on(connection, &token, &plan, &mut out, &mut err)).await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

#[tokio::test]
async fn building_a_base_from_the_shell() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = outside(&home);
    std::fs::write(cwd.join("cat.md"), "# 猫\n\n团子是橘猫。\n").expect("写得进");
    std::fs::write(cwd.join("dog.txt"), "阿黄是小狗。\n").expect("写得进");

    let none = run(&home, &cwd, &[]).await;
    assert_eq!((none.code, none.out.as_str()), (0, "还没有知识库。\n"));
    let made = run(
        &home,
        &cwd,
        &[
            "new",
            "notes",
            "--name",
            "笔记",
            "--description",
            "家里的事",
        ],
    )
    .await;
    assert_eq!(
        (made.code, made.out.as_str()),
        (0, "建好了：notes\n"),
        "{}",
        made.err
    );
    let added = run(
        &home,
        &cwd,
        &["add", "notes", "cat.md", "dog.txt", "--to", "pets"],
    )
    .await;
    assert_eq!(
        (added.code, added.out.as_str()),
        (
            0,
            "加进去了：notes/pets/cat.md\n加进去了：notes/pets/dog.txt\n"
        ),
        "{}",
        added.err
    );
    let again = run(&home, &cwd, &["add", "notes", "cat.md", "--to", "pets"]).await;
    assert_eq!((again.code, again.err.as_str()), (1, "这一篇已经有了。\n"));
    std::fs::write(cwd.join("cat.md"), "# 猫\n\n团子改名叫小橘。\n").expect("写得进");
    let replaced = run(
        &home,
        &cwd,
        &["add", "notes", "cat.md", "--to", "pets", "--replace"],
    )
    .await;
    assert_eq!(replaced.code, 0, "{}", replaced.err);

    let listed = run(&home, &cwd, &["list"]).await;
    assert_eq!(listed.out, "notes  笔记  2 篇  家里的事\n");
    let json = run(&home, &cwd, &["list", "--format", "json"]).await;
    let bases: serde_json::Value = serde_json::from_str(&json.out).expect("是 JSON");
    assert_eq!(bases[0]["documents"], 2);
    let found = run(&home, &cwd, &["search", "小橘"]).await;
    assert_eq!(found.out, "notes  pets/cat.md  猫\n  团子改名叫小橘。\n");
    let missing = run(&home, &cwd, &["search", "鹦鹉"]).await;
    assert_eq!(missing.out, "没找到。\n");
    let read = run(&home, &cwd, &["read", "notes", "pets/dog.txt"]).await;
    assert_eq!(read.out, "阿黄是小狗。\n");
    let edited = run(&home, &cwd, &["edit", "notes", "--name", "家"]).await;
    assert_eq!(edited.out, "改好了：notes\n");
    let nothing = run(&home, &cwd, &["edit", "notes"]).await;
    assert_eq!(
        (nothing.code, nothing.err.as_str()),
        (1, "改什么：写 --name 或者 --description。\n")
    );
    let rebuilt = run(&home, &cwd, &["reindex", "notes"]).await;
    assert_eq!(rebuilt.out, "重建好了：notes，2 篇\n");
    let removed = run(&home, &cwd, &["remove", "notes", "pets/dog.txt"]).await;
    // 有的机器上临时目录那块盘没有能放的回收站：照实报。
    match removed.code {
        0 => assert!(removed.out.starts_with("挪进回收站了："), "{}", removed.out),
        _ => assert!(removed.err.starts_with("挪不进回收站："), "{}", removed.err),
    }
    let unknown = run(&home, &cwd, &["read", "nope", "a.md"]).await;
    assert_eq!(
        (unknown.code, unknown.err.as_str()),
        (1, "知识库不存在。\n")
    );
}

#[tokio::test]
async fn mounting_a_folder_and_deleting_it_from_the_shell() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = outside(&home);
    let notes = cwd.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子\n").expect("写得进");
    let mounted = run(&home, &cwd, &["mount", "mine", "notes"]).await;
    assert_eq!(
        (mounted.code, mounted.out.as_str()),
        (0, "挂上了：mine，1 篇\n"),
        "{}",
        mounted.err
    );
    let gone = run(&home, &cwd, &["mount", "gone", "nowhere"]).await;
    assert_eq!(gone.code, 1);
    assert!(gone.err.starts_with("这个目录挂不上："), "{}", gone.err);
    let refused = run(&home, &cwd, &["remove", "mine", "a.md"]).await;
    assert_eq!(
        (refused.code, refused.err.as_str()),
        (1, "挂进来的知识库是只读的。\n")
    );
    let deleted = run(&home, &cwd, &["delete", "mine"]).await;
    match deleted.code {
        0 => assert!(
            deleted.out.starts_with("删掉了 mine，在回收站里："),
            "{}",
            deleted.out
        ),
        _ => assert!(deleted.err.starts_with("挪不进回收站："), "{}", deleted.err),
    }
    assert_eq!(
        std::fs::read_to_string(notes.join("a.md")).expect("在"),
        "团子\n",
        "挂的目录没动"
    );
}
