//! `miyu session`（施工 SE-1 中上，`docs/blueprint/cli/session.md`）：在进程里起两个核心。一个里 `miyu ask` 说一句，
//! `export` 不写会话、`-o` 写相对路径，包写到当前目录下、标准输出印绝对路径；另一个 `import --archive` 印出新会话的编号，
//! 再导一次什么都不印；写到已经有的文件照核心的话说、退出码 1。`--md`（施工 SE-1 中下下）：默认写到 `.md`、印路径；`-o -`
//! 打到标准输出，`--for`、`--last` 照传；超了上限的照核心的话说、提示改成写文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::support::{Asked, Home, Tape, plan, within};
use miyu_cli::language::Language;
use miyu_cli::{Session, SessionCommand, Target, session_on};
use miyu_session::testkit::{Play, Script};

/// 数据根外面的一个空目录，当「当前目录」。
fn here() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "miyu-cli-session-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

async fn run(home: &Home, command: SessionCommand, here: &Path) -> Asked {
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let args = Session { command };
    let code = within(
        "办完",
        session_on(
            connection,
            &token,
            &args,
            (here, Language::Chinese),
            (&mut out, &mut err),
        ),
    )
    .await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

#[tokio::test]
async fn export_from_one_core_and_import_into_another() {
    let source = Home::new(Arc::new(Script::new([Play::Says("好。")])));
    assert_eq!(source.ask(&plan("hi")).await.code, 0);
    let session = source.sessions()[0].clone();
    let here = here();
    let exported = run(
        &source,
        SessionCommand::Export {
            session: None,
            output: Some(PathBuf::from("out.zip")),
            md: false,
            purpose: None,
            last: None,
        },
        &here,
    )
    .await;
    assert_eq!(exported.code, 0, "{}", exported.err);
    let written = here.join("out.zip");
    assert_eq!(
        exported.out,
        format!("{}\n", written.display()),
        "印绝对路径"
    );
    assert!(written.is_file());

    let target = Home::new(Arc::new(Script::new([])));
    let imported = run(
        &target,
        SessionCommand::Import {
            file: PathBuf::from("out.zip"),
            archive: true,
        },
        &here,
    )
    .await;
    assert_eq!(imported.code, 0, "{}", imported.err);
    assert_eq!(imported.out, format!("{}\n", session.as_str()));
    let again = run(
        &target,
        SessionCommand::Import {
            file: written.clone(),
            archive: true,
        },
        &here,
    )
    .await;
    assert_eq!((again.code, again.screen.as_str()), (0, ""), "导过的不再印");

    let taken = run(
        &source,
        SessionCommand::Export {
            session: Some(session.as_str().to_string()),
            output: Some(written),
            md: false,
            purpose: None,
            last: None,
        },
        &here,
    )
    .await;
    assert_eq!(taken.code, 1);
    assert!(taken.err.contains("文件已存在"), "{}", taken.err);
    std::fs::remove_dir_all(&here).expect("删得掉当前目录");
}

/// `miyu session export --md`：参数照 `SessionCommand::Export` 写。
fn md(
    session: Option<&str>,
    output: Option<&str>,
    purpose: Option<&str>,
    last: Option<u32>,
) -> SessionCommand {
    SessionCommand::Export {
        session: session.map(str::to_string),
        output: output.map(PathBuf::from),
        md: true,
        purpose: purpose.map(str::to_string),
        last,
    }
}

#[tokio::test]
async fn markdown_goes_to_a_file_or_to_standard_output() {
    let home = Home::new(Arc::new(Script::new([
        Play::Says("第一句的回答。"),
        Play::Says("第二句的回答。"),
    ])));
    assert_eq!(home.ask(&plan("第一句")).await.code, 0);
    let session = home.sessions()[0].clone();
    let mut again = plan("第二句");
    again.target = Target::Continue;
    assert_eq!(home.ask(&again).await.code, 0);
    let here = here();
    let written = run(&home, md(None, None, None, None), &here).await;
    assert_eq!(written.code, 0, "{}", written.err);
    let file = here.join(format!(
        "miyu-session-{}.md",
        session.as_str().chars().take(8).collect::<String>()
    ));
    assert_eq!(written.out, format!("{}\n", file.display()), "印绝对路径");
    let text = std::fs::read_to_string(&file).expect("写出来了");
    assert!(
        text.contains("## User\n\n第一句") && text.contains("第二句的回答。"),
        "{text}"
    );

    let printed = run(
        &home,
        md(Some(session.as_str()), Some("-"), Some("agent"), Some(1)),
        &here,
    )
    .await;
    assert_eq!(printed.code, 0, "{}", printed.err);
    assert!(
        printed.out.starts_with("# Session transcript\n"),
        "{}",
        printed.out
    );
    assert!(
        printed.out.contains("1 earlier turn is left out") && !printed.out.contains("第一句"),
        "--last 1 只要最近一轮：{}",
        printed.out
    );
    assert!(
        printed.out.ends_with("第二句的回答。\n"),
        "原样打出来：{}",
        printed.out
    );
    assert!(!here.join("-").exists(), "不写文件");
    std::fs::remove_dir_all(&here).expect("删得掉当前目录");
}

#[tokio::test]
async fn markdown_too_big_for_standard_output_says_to_save_a_file() {
    let home = Home::new(Arc::new(Script::new([Play::Says("好。")])));
    assert_eq!(home.ask(&plan(&"y".repeat(600 * 1024))).await.code, 0);
    let here = here();
    let printed = run(&home, md(None, Some("-"), None, None), &here).await;
    assert_eq!(printed.code, 1);
    assert_eq!(printed.out, "", "标准输出上什么都没有");
    assert!(
        printed.err.contains("导出超过 512 KiB")
            && printed.err.contains("提示：用 -o <文件> 存成文件"),
        "{}",
        printed.err
    );
    std::fs::remove_dir_all(&here).expect("删得掉当前目录");
}
