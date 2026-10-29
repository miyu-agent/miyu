//! `miyu pkg install` 从软件源装分阶段印（施工 IN-3，设计 `49-装包照pacman.md` 第二节）：在进程里起的核心认测试的钥匙，假服务器
//! 当镜像。管道里（标准错误不是终端）不画进度条、一个包一行；列推荐的、单独问一句，答「是」的主包装完再装；`--yes` 不装推荐的。

use serde_json::json;

use crate::pkg_source::{entry, home_with_mirror, keys, package, pkg_speaking};
use crate::support::Home;
use miyu_cli::language::Language;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_source::sign;

/// 照 pacman 写的大小（同 `miyu pkg` 的）：包文件都不到 1 KiB。
fn human(bytes: usize) -> String {
    assert!(bytes < 1024, "测试的包文件不到 1 KiB：{bytes}");
    format!("{bytes} B")
}

/// 一个装 `app`（依赖 `lib`，推荐 `rec`）的软件源：交回核心、三个包文件的字节数。服务器照先后回索引、签名、`lib`、`app`、`rec`。
async fn source(tag: &str) -> (Home, Server, [usize; 3]) {
    let (secret, trusted) = keys();
    let scratch =
        std::env::temp_dir().join(format!("miyu-cli-stages-{tag}-{}", std::process::id()));
    let app = package(&scratch, "app", &["lib"]);
    let lib = package(&scratch, "lib", &[]);
    let rec = package(&scratch, "rec", &[]);
    let mut app_entry = entry("app", &["lib"], &app);
    app_entry["recommends"] = json!(["rec"]);
    app_entry["installed"] = json!(4096);
    let mut lib_entry = entry("lib", &[], &lib);
    lib_entry["installed"] = json!(2048);
    let index = serde_json::to_vec(&json!({"version": 1, "generated": "2026-10-19T03:00:00Z",
        "packages": [app_entry, lib_entry, entry("rec", &[], &rec)]}))
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, &secret).expect("签得了")).expect("写得成");
    let bytes = |body: &Vec<u8>| Reply::stream(vec![Piece::Bytes(body.clone())]);
    let server = Server::start(vec![
        bytes(&index),
        bytes(&signature),
        bytes(&lib.1),
        bytes(&app.1),
        bytes(&rec.1),
    ])
    .await;
    let mirror = format!("{}/m", server.base_url);
    let home = home_with_mirror("https://unreachable.invalid/", &mirror, trusted);
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("临时目录没删掉：{error}");
    }
    (home, server, [lib.1.len(), app.1.len(), rec.1.len()])
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
async fn a_pipe_shows_each_stage_and_installs_the_recommended_after() {
    let (home, _server, [lib, app, rec]) = source("zh").await;
    let (code, tape) = pkg_speaking(&home, &["-S", "app"], b"y\ny\n", Language::Chinese).await;
    let screen = tape.text(|_| true);
    assert_eq!(code, 0, "{screen}");
    let expected = format!(
        "正在解析依赖…\n正在检查冲突…\n\
         软件包 (2)  lib-1.0  app-1.0\n推荐 (1)    rec-1.0\n\n\
         下载大小    : {}\n安装后大小  : 6.0 KiB\n来源        : 官方，签名已验证\n\n\
         安装 2 个软件包？[Y/n] 同时安装推荐的 1 个软件包？[Y/n] \
         正在下载…\n已下载 lib-1.0 {}\n已下载 app-1.0 {}\n\
         (1/2) 正在核对完整性\n(2/2) 正在核对完整性\n(1/2) 正在安装 lib\n(2/2) 正在安装 app\n\
         已安装 lib\n已安装 app 1.0\n\
         正在下载…\n已下载 rec-1.0 {}\n(1/1) 正在核对完整性\n(1/1) 正在安装 rec\n已安装 rec 1.0\n",
        human(lib + app),
        human(lib),
        human(app),
        human(rec),
    );
    assert_eq!(screen, expected);
    assert!(
        !screen.contains('\r') && !screen.contains("[#"),
        "管道里不画进度条"
    );
    assert!(installed(&home, "rec"), "答了「是」的推荐的装上了");
}

#[tokio::test]
async fn yes_installs_the_packages_but_not_the_recommended() {
    let (home, _server, [lib, app, _]) = source("en").await;
    let (code, tape) = pkg_speaking(&home, &["-S", "app", "--yes"], b"", Language::English).await;
    let screen = tape.text(|_| true);
    assert_eq!(code, 0, "{screen}");
    let expected = format!(
        "resolving dependencies...\nlooking for conflicting packages...\n\
         downloading packages...\ndownloaded lib-1.0 {}\ndownloaded app-1.0 {}\n\
         (1/2) checking package integrity\n(2/2) checking package integrity\n\
         (1/2) installing lib\n(2/2) installing app\n\
         installed lib\ninstalled app 1.0\n",
        human(lib),
        human(app),
    );
    assert_eq!(screen, expected);
    assert!(
        installed(&home, "app") && !installed(&home, "rec"),
        "{screen}"
    );
}

#[tokio::test]
async fn saying_no_to_the_recommended_still_installs_the_packages() {
    let (home, _server, _) = source("no").await;
    let (code, tape) = pkg_speaking(&home, &["install", "app"], b"\nn\n", Language::Chinese).await;
    let out: String = tape.text(|err| !err);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("安装 2 个软件包？[Y/n] 同时安装推荐的 1 个软件包？[Y/n] 已安装 lib\n"),
        "{out}"
    );
    assert!(installed(&home, "app") && !installed(&home, "rec"));
    let err = tape.text(|err| err);
    assert!(!err.contains("rec-1.0"), "没装的推荐的不下载：{err}");
}
