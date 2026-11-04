//! `miyu session`（施工 SE-1 中上，`docs/blueprint/cli/session.md`）：在进程里起两个核心。一个里 `miyu ask` 说一句，
//! `export` 不写会话、`-o` 写相对路径，包写到当前目录下、标准输出印绝对路径；另一个 `import --archive` 印出新会话的编号，
//! 再导一次什么都不印；写到已经有的文件照核心的话说、退出码 1。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::support::{Asked, Home, Tape, plan, within};
use miyu_cli::language::Language;
use miyu_cli::{Session, SessionCommand, session_on};
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
        },
        &here,
    )
    .await;
    assert_eq!(taken.code, 1);
    assert!(taken.err.contains("文件已存在"), "{}", taken.err);
    std::fs::remove_dir_all(&here).expect("删得掉当前目录");
}
