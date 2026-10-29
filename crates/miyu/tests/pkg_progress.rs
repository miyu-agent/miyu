//! `miyu pkg install` 的进度条（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 4 条）：真的 `miyu core`、真的 `miyu pkg`，假服务器
//! 当配置里另加的软件源。标准错误是伪终端的画进度条、照终端宽度、刷新同一行，`--yes` 不装推荐的；接的是管道的只印行。只在
//! Unix 上跑：Windows 没有伪终端。

#![cfg(unix)]

use std::process::Command;

use serde_json::{Value, json};

use crate::support::cli::{run, stderr, stdout};
use crate::support::pty::{Capture, pty, secondary};
use crate::support::{Home, MIYU, offline, resources, within};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_ipc::connect_or_start;
use miyu_source::sign;

/// 伪终端多宽。
const COLUMNS: u16 = 60;

/// 拉起来的核心、两个假服务器（一个回包文件，一个当软件源回索引、签名）。
struct Source {
    home: Home,
    _files: Server,
    _index: Server,
}

/// 一个数据根：系统配置里官方的源到不了，另加一个叫 `test` 的源，装 `app`（依赖 `lib`，推荐 `rec`）。别的源没有镜像，包文件
/// 照索引写的地址下：回包文件的服务器照先后回 `lib`、`app`。核心拉起来了。
async fn source() -> Source {
    let home = Home::new();
    let made = sign::generate().expect("生成得了");
    let scratch = home.dir.with_extension("pkgsrc");
    let mut packed = Vec::new();
    for (id, depends) in [
        ("lib", ""),
        ("app", "[depends]\npackages = [\"lib\"]\n"),
        ("rec", ""),
    ] {
        let folder = scratch.join(id);
        std::fs::create_dir_all(&folder).expect("建得了");
        let manifest =
            format!("[package]\nversion = \"1.0\"\nname = {{ en = \"{id}\" }}\n\n{depends}");
        std::fs::write(folder.join("package.toml"), manifest).expect("写得进");
        let out = scratch.join(miyu_source::archive::file_name(id, "1.0", None));
        let made = miyu_source::archive::pack(&folder, id, &out).expect("打得成");
        packed.push((id, made, std::fs::read(&out).expect("在")));
    }
    let reply = |bytes: &[u8]| Reply::stream(vec![Piece::Bytes(bytes.to_vec())]);
    let files = Server::start(vec![reply(&packed[0].2), reply(&packed[1].2)]).await;
    let entries: Vec<Value> = packed
        .iter()
        .map(|(id, made, _)| {
            let depends = if *id == "app" { json!(["lib"]) } else { json!([]) };
            let recommends = if *id == "app" { json!(["rec"]) } else { json!([]) };
            json!({"id": id, "version": "1.0", "name": {"en": id},
                   "origin": format!("github.com/someone/{id}"), "depends": depends, "recommends": recommends,
                   "files": [{"url": format!("{}/files/{id}-1.0.miyu", files.base_url),
                              "size": made.size, "sha256": made.sha256}]})
        })
        .collect();
    let index = serde_json::to_vec(
        &json!({"version": 1, "generated": "2026-10-19T03:00:00Z", "packages": entries}),
    )
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, &made.secret).expect("签得了")).expect("写得成");
    let feed = Server::start(vec![reply(&index), reply(&signature)]).await;
    home.system_config(&format!(
        "[packages]\nsource = \"https://unreachable.invalid/\"\nmirrors = []\n\n\
         [packages.sources.test]\nurl = \"{}/\"\nkey = \"{}\"\n",
        feed.base_url, made.public
    ));
    let (_held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("临时目录没删掉：{error}");
    }
    Source {
        home,
        _files: files,
        _index: feed,
    }
}

fn installed(home: &Home, id: &str) -> bool {
    home.root
        .path()
        .join("packages")
        .join(id)
        .join("package.toml")
        .is_file()
}

#[tokio::test]
async fn a_terminal_gets_a_bar_as_wide_as_it_is_and_yes_skips_the_recommended() {
    let source = source().await;
    let home = &source.home;
    let (main, slave) = pty();
    let size = rustix::termios::Winsize {
        ws_row: 24,
        ws_col: COLUMNS,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    rustix::termios::tcsetwinsize(&main, size).expect("设得了窗口大小");
    let mut child = Command::new(MIYU)
        .args(["pkg", "-S", "app", "--yes"])
        .env("MIYU_HOME", home.root.path())
        .envs(offline(home.root.path()))
        .env("MIYU_RESOURCES", resources())
        .env("LANG", "zh_CN.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .stdin(secondary(&slave))
        .stdout(secondary(&slave))
        .stderr(secondary(&slave))
        .spawn()
        .expect("跑得起来");
    let capture = Capture::start(&main);
    let (code, screen) = tokio::task::spawn_blocking(move || capture.finish(&mut child))
        .await
        .expect("没 panic");
    assert_eq!(code, 0, "{screen}");
    let bars: Vec<&str> = screen
        .split("\x1b[2K")
        .filter(|piece| piece.starts_with(" lib-1.0") || piece.starts_with(" app-1.0"))
        .map(|piece| piece.trim_end_matches('\n'))
        .collect();
    let full: Vec<&&str> = bars.iter().filter(|bar| bar.ends_with("] 100%")).collect();
    assert_eq!(full.len(), 2, "两个包各一条，刷到 100%：{screen:?}");
    for bar in &full {
        assert!(bar.contains("[####"), "{bar}");
        assert_eq!(
            bar.chars().count(),
            usize::from(COLUMNS) - 1,
            "照终端宽度画，最后一格留着：{bar}"
        );
    }
    assert!(
        screen.contains("(2/2) 正在核对完整性\n(1/2) 正在安装 lib\n(2/2) 正在安装 app\n"),
        "{screen:?}"
    );
    assert!(
        screen.ends_with("已安装 lib\n已安装 app 1.0\n"),
        "{screen:?}"
    );
    assert!(!screen.contains("推荐"), "--yes 不问推荐的：{screen:?}");
    assert!(installed(home, "app") && !installed(home, "rec"));
}

#[tokio::test]
async fn a_pipe_gets_lines_and_no_bar() {
    let source = source().await;
    let home = &source.home;
    let cwd = home.dir.with_extension("pipe");
    std::fs::create_dir_all(&cwd).expect("建得了");
    let done = run(
        home.root.path(),
        &cwd,
        "zh_CN.UTF-8",
        &["pkg", "-S", "app", "--yes"],
    )
    .await;
    assert_eq!(done.status.code(), Some(0), "{done:?}");
    let err = stderr(&done);
    assert!(err.contains("正在下载…\n已下载 lib-1.0 "), "{err}");
    assert!(err.contains("\n已下载 app-1.0 "), "{err}");
    assert!(
        !err.contains('\r') && !err.contains("\x1b[") && !err.contains("[#"),
        "{err:?}"
    );
    assert_eq!(stdout(&done), "已安装 lib\n已安装 app 1.0\n");
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}
