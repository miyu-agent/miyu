//! `miyu kb`（施工 K-7 下，`docs/blueprint/cli/kb.md`）：在进程里起一个核心，每个子命令走一遍：印什么、退出码；加文件照本机
//! 路径传上去再加进库、`--to` 放进库里的目录、已经有的拒、`--replace` 换；挂目录照当前目录接相对路径；核心拒的照原话说，挂不上的
//! 接上为什么；`edit` 什么都没写的说一句、退出码 1。

use std::path::Path;
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
async fn each_home_has_its_own_folder_beside_the_data_root() {
    // 原来照数据根的上一级拼，所有测试共用一个 `<临时目录>/outside`：并着跑时别的测试把 `cat.md` 写回原样，换的那一篇偶尔
    // 换成旧的。
    let (a, b) = (
        Home::new(Arc::new(Script::new([]))),
        Home::new(Arc::new(Script::new([]))),
    );
    assert_ne!(a.outside(), b.outside());
}

#[tokio::test]
async fn building_a_base_from_the_shell() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
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
        (
            1,
            "改什么：写 --name、--description、--auto 或者 --no-auto。\n"
        )
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
    let cwd = home.outside();
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

#[tokio::test]
async fn the_auto_switch_from_the_shell() {
    // 施工 K-6 补：`--auto` 开、`--no-auto` 关，`new`、`edit`、`mount` 都收；列表在篇数后面接「自动想起」；只写开关的 `edit` 也算
    // 改了；两个一起写读不懂。
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    let toml = |kb: &str| {
        std::fs::read_to_string(
            home.root
                .account_dir(&crate::support::AccountIdOf::admin())
                .join("kb")
                .join(kb)
                .join("kb.toml"),
        )
        .expect("读得了")
    };
    let made = run(&home, &cwd, &["new", "world", "--name", "雾港", "--auto"]).await;
    assert_eq!(made.code, 0, "{}", made.err);
    assert!(toml("world").contains("auto = true"), "{}", toml("world"));
    let listed = run(&home, &cwd, &[]).await;
    assert_eq!(listed.out, "world  雾港  0 篇  自动想起\n");
    let off = run(&home, &cwd, &["edit", "world", "--no-auto"]).await;
    assert_eq!(
        (off.code, off.out.as_str()),
        (0, "改好了：world\n"),
        "{}",
        off.err
    );
    assert!(!toml("world").contains("auto"), "{}", toml("world"));
    assert_eq!(run(&home, &cwd, &[]).await.out, "world  雾港  0 篇\n");
    let on = run(&home, &cwd, &["edit", "world", "--auto"]).await;
    assert_eq!(on.code, 0, "{}", on.err);
    assert!(toml("world").contains("auto = true"));

    let notes = cwd.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子\n").expect("写得进");
    let mounted = run(&home, &cwd, &["mount", "mine", "notes", "--auto"]).await;
    assert_eq!(mounted.code, 0, "{}", mounted.err);
    assert!(toml("mine").contains("auto = true"), "{}", toml("mine"));

    let both = Line::try_parse_from(["miyu", "kb", "edit", "world", "--auto", "--no-auto"]);
    assert!(both.is_err(), "两个一起写读不懂");
}

#[tokio::test]
async fn what_the_core_would_refuse_is_said_plainly() {
    // 施工 K-7 修（验收手册待定 2 到 4）：原来只说「参数错误。」。不认的格式在传以前就说清只收哪几种，一个都不传；库的编号写错
    // 说清写法，退出码 2；页码出界说一共几页。
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    std::fs::write(cwd.join("cat.md"), "# 猫\n\n团子是橘猫。\n").expect("写得进");
    std::fs::write(cwd.join("report.docx"), "PK").expect("写得进");
    assert_eq!(run(&home, &cwd, &["new", "notes"]).await.code, 0);

    let docx = run(&home, &cwd, &["add", "notes", "cat.md", "report.docx"]).await;
    assert_eq!(
        (docx.code, docx.out.as_str(), docx.err.as_str()),
        (
            1,
            "",
            "只收 .md、.markdown、.txt、.pdf、.html、.htm：report.docx\n"
        ),
        "一个都不传"
    );
    assert_eq!(run(&home, &cwd, &[]).await.out, "notes  0 篇\n");

    let rule = "库的编号要小写字母开头，只能有小写字母、数字、-、_，最多 64 个：日语\n";
    let named = run(&home, &cwd, &["new", "日语"]).await;
    assert_eq!((named.code, named.err.as_str()), (2, rule));
    let searched = run(&home, &cwd, &["search", "团子", "--kb", "日语"]).await;
    assert_eq!((searched.code, searched.err.as_str()), (2, rule));

    // 一页 32 KiB：这一篇两页。
    std::fs::write(cwd.join("long.md"), "一".repeat(12_000)).expect("写得进");
    assert_eq!(run(&home, &cwd, &["add", "notes", "long.md"]).await.code, 0);
    let page = run(&home, &cwd, &["read", "notes", "long.md", "--page", "9"]).await;
    assert_eq!(
        (page.code, page.err.as_str()),
        (1, "没有第 9 页，一共 2 页。\n")
    );
    let missing = run(&home, &cwd, &["read", "notes", "dog.md", "--page", "2"]).await;
    assert_eq!(
        (missing.code, missing.err.as_str()),
        (1, "文档不存在。\n"),
        "别的错照原话"
    );
}

#[tokio::test]
async fn a_base_from_a_package_from_the_shell() {
    // 施工 K-8：装上一个带 kb/world/ 的包，列表接「随 <包名>」；只开关自动想起改得了，改名字照核心的原话拒。
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    let pack = cwd.join("world-pack");
    std::fs::create_dir_all(pack.join("kb/world")).expect("建得了");
    std::fs::write(
        pack.join("package.toml"),
        "[package]\nname = { en = \"World\", zh = \"雾港世界观\" }\n",
    )
    .expect("写得进");
    std::fs::write(
        pack.join("kb/world/kb.toml"),
        "name = \"雾港\"\nauto = true\n",
    )
    .expect("写得进");
    std::fs::write(pack.join("kb/world/people.md"), "# 艾琳\n\n守灯人。\n").expect("写得进");
    let installed = {
        #[derive(Parser)]
        struct PkgLine {
            #[command(subcommand)]
            command: PkgTop,
        }
        #[derive(clap::Subcommand)]
        enum PkgTop {
            Pkg(miyu_cli::Pkg),
        }
        let PkgTop::Pkg(args) = PkgLine::try_parse_from([
            "miyu",
            "pkg",
            "install",
            "./world-pack",
            "--yes",
            "--system",
        ])
        .expect("读得懂")
        .command;
        let plan = miyu_cli::PkgPlan {
            args,
            language: Language::Chinese,
            cwd: cwd.clone(),
        };
        let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
        let tape = Tape::default();
        let (mut out, mut err) = (tape.pen(false), tape.pen(true));
        let code = within(
            "装",
            miyu_cli::pkg_on(
                connection,
                &token,
                &plan,
                &mut std::io::empty(),
                &mut out,
                &mut err,
            ),
        )
        .await;
        (code, tape.text(|_| true))
    };
    assert_eq!(installed.0, 0, "{}", installed.1);
    let listed = run(&home, &cwd, &[]).await;
    assert_eq!(
        listed.out, "world  雾港  1 篇  随 雾港世界观  自动想起\n",
        "{}",
        listed.err
    );
    let off = run(&home, &cwd, &["edit", "world", "--no-auto"]).await;
    assert_eq!(
        (off.code, off.out.as_str()),
        (0, "改好了：world\n"),
        "{}",
        off.err
    );
    assert_eq!(
        run(&home, &cwd, &[]).await.out,
        "world  雾港  1 篇  随 雾港世界观\n"
    );
    let renamed = run(&home, &cwd, &["edit", "world", "--name", "新名字"]).await;
    assert_eq!(
        (renamed.code, renamed.err.as_str()),
        (1, "随软件包来的知识库是只读的，只能开关自动想起。\n")
    );
}

/// `list <库>` 列文档（施工 K-10）：一篇一行，读不了的接为什么；空的库说一句；`--format json` 原样印整个回应。
#[tokio::test]
async fn the_documents_of_a_base_from_the_shell() {
    let home = Home::new(Arc::new(Script::new([])));
    let cwd = home.outside();
    let notes = cwd.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子\n").expect("写得进");
    std::fs::write(notes.join("broken.txt"), [0xff, 0xfe]).expect("写得进");
    assert_eq!(run(&home, &cwd, &["mount", "mine", "notes"]).await.code, 0);
    let listed = run(&home, &cwd, &["list", "mine"]).await;
    assert_eq!(listed.code, 0, "{}", listed.err);
    let rows: Vec<&str> = listed.out.lines().collect();
    assert_eq!(rows.len(), 2, "{}", listed.out);
    assert!(rows[0].starts_with("a.md        7 B  20"), "{}", rows[0]);
    assert!(
        rows[1].starts_with("broken.txt  2 B  20")
            && rows[1].ends_with("  （读不了：不是 UTF-8 编码）"),
        "{}",
        rows[1]
    );
    let json = run(&home, &cwd, &["list", "mine", "--format", "json"]).await;
    let value: serde_json::Value = serde_json::from_str(json.out.trim()).expect("是 JSON");
    assert_eq!(value["documents"][1]["reason"], "not_utf8", "{}", json.out);
    assert_eq!(run(&home, &cwd, &["new", "empty"]).await.code, 0);
    let empty = run(&home, &cwd, &["list", "empty"]).await;
    assert_eq!(
        (empty.code, empty.out.as_str()),
        (0, "这个库里还没有文档。\n")
    );
    let bad = run(&home, &cwd, &["list", "Bad Id"]).await;
    assert_eq!(bad.code, 2, "库的编号写错的说清写法：{}", bad.err);
}
