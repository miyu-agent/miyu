//! 真跑 `miyu pkg`（施工 T-3，`docs/blueprint/cli/pkg.md`）：列出、照相对路径装一份清单、卸掉；卸掉出厂的标「已卸载」、照编号
//! 装回来；必需的卸不掉、写错的清单装不上，退出码 1；`--format json` 原样印那一串；`-h` 印那一页。看一个包的信息、装了哪些
//! 文件、文件归哪个包、文件改没改，pacman 的写法也认（施工 F-8 下）。

use std::path::Path;
use std::process::Output;

use serde_json::Value;

use crate::support::cli::{command, run, stderr, stdout};
use crate::support::{Home, run_starting, within};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;

/// 一份手动拉起的扩展包的清单（不会真的起）。
const XPKG: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"X\", zh = \"测试包\" }\n\n[command]\nname = \"xpkg\"\nprogram = \"miyu-nothing\"\nabout = { en = \"X\" }\n\n[process]\nstart = \"manual\"\n";

/// 列出来的那几行里，编号是 `id` 的那一行。
fn line<'a>(listed: &'a str, id: &str) -> Option<&'a str> {
    listed
        .lines()
        .find(|line| line.split_whitespace().next() == Some(id))
}

#[tokio::test]
async fn pkg_lists_installs_and_removes_through_a_real_core() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("work");
    std::fs::create_dir_all(&cwd).expect("建得了目录");
    for (id, text) in [("xpkg", XPKG), ("bad", "[package]\n\n[process]\n")] {
        std::fs::create_dir_all(cwd.join(id)).expect("建得了目录");
        std::fs::write(cwd.join(id).join("package.toml"), text).expect("写得进");
    }
    let zh = "zh_CN.UTF-8";

    let listed = run(&root, &cwd, zh, &["pkg"]).await;
    assert_eq!(listed.status.code(), Some(0), "{listed:?}");
    let text = stdout(&listed);
    assert!(
        line(&text, "shell").is_some_and(|line| line.ends_with("运行命令")),
        "{text}"
    );

    let installed = run(&root, &cwd, zh, &["pkg", "install", "./xpkg", "--yes"]).await;
    assert_eq!(stdout(&installed), "装好了：xpkg\n", "{installed:?}");
    let text = stdout(&run(&root, &cwd, zh, &["pkg", "list"]).await);
    assert!(
        line(&text, "xpkg").is_some_and(|line| line.ends_with("测试包")),
        "{text}"
    );
    let removed = run(&root, &cwd, zh, &["pkg", "remove", "xpkg", "--yes"]).await;
    assert_eq!(stdout(&removed), "卸掉了：xpkg\n", "{removed:?}");
    assert!(line(&stdout(&run(&root, &cwd, zh, &["pkg"]).await), "xpkg").is_none());

    // 出厂的：卸掉的照样列、标「已卸载」，照编号装回来。
    run(&root, &cwd, zh, &["pkg", "remove", "net", "--yes"]).await;
    let text = stdout(&run(&root, &cwd, zh, &["pkg"]).await);
    assert!(
        line(&text, "net").is_some_and(|line| line.ends_with("（已卸载）")),
        "{text}"
    );
    let back = run(&root, &cwd, zh, &["pkg", "install", "net", "--yes"]).await;
    assert_eq!(stdout(&back), "装好了：net\n", "{back:?}");

    // 施工 BS-1 上中起没有必需的包；卸没有的照旧拒。
    let unknown = run(&root, &cwd, zh, &["pkg", "remove", "nope", "--yes"]).await;
    assert_eq!(unknown.status.code(), Some(1));
    assert_eq!(stderr(&unknown), "miyu: 软件包不存在\n");
    let bad = run(&root, &cwd, zh, &["pkg", "install", "bad/", "--yes"]).await;
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        stderr(&bad).starts_with("miyu: 装不上："),
        "{}",
        stderr(&bad)
    );

    let json = run(&root, &cwd, zh, &["pkg", "--format", "json"]).await;
    let packages: Value = serde_json::from_str(&stdout(&json)).expect("是 JSON");
    assert!(
        packages
            .as_array()
            .is_some_and(|all| all.iter().any(|one| one["package"] == "shell")),
        "{packages}"
    );
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

#[tokio::test]
async fn pkg_tells_about_installed_files_through_a_real_core() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("query");
    std::fs::create_dir_all(cwd.join("xpkg/bin")).expect("建得了目录");
    std::fs::write(cwd.join("xpkg/package.toml"), XPKG).expect("写得进");
    std::fs::write(cwd.join("xpkg/bin/data.txt"), "1").expect("写得进");
    let zh = "zh_CN.UTF-8";

    let installed = run(&root, &cwd, zh, &["pkg", "-U", "./xpkg", "--yes"]).await;
    assert_eq!(
        stdout(&installed),
        "装好了：xpkg\n",
        "pacman 的 -U：{installed:?}"
    );
    let info = stdout(&run(&root, &cwd, zh, &["pkg", "info", "xpkg"]).await);
    assert!(info.starts_with("名称      测试包\n"), "{info}");
    assert!(
        info.contains("个文件") && info.contains("安装时间  "),
        "{info}"
    );

    let files = stdout(&run(&root, &cwd, zh, &["pkg", "-Ql", "xpkg"]).await);
    let data = files
        .lines()
        .find_map(|line| line.strip_prefix("xpkg "))
        .filter(|path| path.ends_with("data.txt"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| panic!("一个文件一行，绝对路径：{files}"));
    assert!(data.is_absolute() && data.exists(), "{files}");
    let owned = run(&root, &cwd, zh, &["pkg", "-Qo", &data.to_string_lossy()]).await;
    assert_eq!(stdout(&owned), format!("{} 属于 xpkg\n", data.display()));
    let nobody = run(&root, &cwd, zh, &["pkg", "owns", "nothing.txt"]).await;
    assert_eq!(nobody.status.code(), Some(1));
    assert!(
        stderr(&nobody).starts_with("没有软件包包含 "),
        "{}",
        stderr(&nobody)
    );

    let fine = run(&root, &cwd, zh, &["pkg", "-Qk"]).await;
    assert_eq!(
        (fine.status.code(), stdout(&fine)),
        (Some(0), "xpkg：正常\n".to_string())
    );
    std::fs::write(&data, "2").expect("写得进");
    let changed = run(&root, &cwd, zh, &["pkg", "check", "xpkg"]).await;
    assert_eq!(
        (changed.status.code(), stdout(&changed)),
        (Some(1), "xpkg：已修改 bin/data.txt\n".to_string())
    );
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 同 [`run`]，标准输入写 `input` 再关上（施工 F-8 下补：装、卸之前问的答）。
async fn answering(root: &Path, cwd: &Path, args: &[&str], input: &str) -> Output {
    let command = command(root, cwd, "zh_CN.UTF-8", args);
    let (dir, input) = (cwd.to_path_buf(), input.to_string());
    tokio::task::spawn_blocking(move || run_starting(&dir, command, &input))
        .await
        .expect("没 panic")
}

#[tokio::test]
async fn pkg_shows_what_it_will_do_and_asks_first() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("ask");
    std::fs::create_dir_all(cwd.join("xpkg")).expect("建得了目录");
    std::fs::write(cwd.join("xpkg/package.toml"), XPKG).expect("写得进");
    let zh = "zh_CN.UTF-8";
    let asked = "\n继续安装？[Y/n] ";

    let no = answering(&root, &cwd, &["pkg", "install", "./xpkg"], "n\n").await;
    let shown = stdout(&no);
    assert!(
        shown.starts_with("将安装 xpkg\n包含：扩展程序、命令 miyu xpkg\n大小：")
            && shown.ends_with(asked),
        "{shown}"
    );
    assert_eq!(
        (no.status.code(), stderr(&no)),
        (Some(1), "已取消\n".to_string())
    );
    assert!(
        line(&stdout(&run(&root, &cwd, zh, &["pkg"]).await), "xpkg").is_none(),
        "答了不就没装"
    );

    let yes = answering(&root, &cwd, &["pkg", "install", "./xpkg"], "\n").await;
    assert!(
        stdout(&yes).ends_with(&format!("{asked}装好了：xpkg\n")),
        "{yes:?}"
    );

    let closed = run(&root, &cwd, zh, &["pkg", "remove", "xpkg"]).await;
    assert_eq!(closed.status.code(), Some(1));
    assert!(
        stdout(&closed).starts_with("将卸载 xpkg\n大小："),
        "{closed:?}"
    );
    assert_eq!(stderr(&closed), "已取消：没有确认（不问用 --yes）\n");
    let gone = run(&root, &cwd, zh, &["pkg", "-R", "xpkg", "--noconfirm"]).await;
    assert_eq!(
        stdout(&gone),
        "卸掉了：xpkg\n",
        "pacman 的写法不问：{gone:?}"
    );
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

#[tokio::test]
async fn every_help_is_the_page_in_the_language() {
    let home = Home::new();
    let cwd = std::env::temp_dir();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["pkg", "-h"][..],
            &["pkg", "install", "-h"],
            &["pkg", "remove", "-h"],
            &["pkg", "info", "-h"],
            &["pkg", "-Qk", "-h"],
            &["help", "pkg"],
        ] {
            let printed = run(home.root.path(), &cwd, lang, args).await;
            assert_eq!(stdout(&printed), page(language, Page::Pkg), "{args:?}");
        }
    }
}

/// 一份带 MCP 服务的 `bundle`（有开关、装好是关着的；服务的程序不在，起不来，不碍开关）。
const TOOLS: &str = "[package]\nname = { en = \"Tools\", zh = \"工具包\" }\n\n[[mcp]]\nname = \"nothing\"\ncommand = [\"miyu-nothing\"]\n";

#[tokio::test]
async fn pkg_offers_to_turn_on_what_it_installed() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("open");
    let fresh = TOOLS.replace("nothing", "other");
    for (id, text) in [("tools", TOOLS), ("xpkg", XPKG), ("fresh", fresh.as_str())] {
        std::fs::create_dir_all(cwd.join(id)).expect("建得了目录");
        std::fs::write(cwd.join(id).join("package.toml"), text).expect("写得进");
    }
    let zh = "zh_CN.UTF-8";
    let open = "\n现在打开？[Y/n] ";

    let no = answering(&root, &cwd, &["pkg", "install", "./tools"], "\nn\n").await;
    assert_eq!(no.status.code(), Some(0), "照样算装好了：{no:?}");
    assert!(
        stdout(&no).ends_with(&format!(
            "装好了：tools\n{open}没有打开：以后在「软件包」页里打开\n"
        )),
        "{no:?}"
    );
    let closed = answering(&root, &cwd, &["pkg", "install", "./tools"], "\n").await;
    assert!(
        stdout(&closed).ends_with(&format!(
            "装好了：tools\n{open}\n没有打开：以后在「软件包」页里打开\n"
        )),
        "没得答的不打开：{closed:?}"
    );
    let yes = answering(&root, &cwd, &["pkg", "install", "./tools"], "\n\n").await;
    assert!(
        stdout(&yes).ends_with(&format!("装好了：tools\n{open}打开了：tools\n")),
        "{yes:?}"
    );
    let again = answering(&root, &cwd, &["pkg", "install", "./tools"], "\n").await;
    assert!(
        stdout(&again).ends_with("装好了：tools\n"),
        "开着的不再问：{again:?}"
    );
    let quiet = run(&root, &cwd, zh, &["pkg", "install", "./fresh", "--yes"]).await;
    assert_eq!(
        stdout(&quiet),
        "装好了：fresh\n打开了：fresh\n",
        "--yes 当是：{quiet:?}"
    );
    let missing = run(&root, &cwd, zh, &["pkg", "install", "./xpkg", "--yes"]).await;
    assert_eq!(
        (missing.status.code(), stdout(&missing)),
        (Some(0), "装好了：xpkg\n".to_string()),
        "程序不在的不问也不开"
    );
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 要能力的扩展打开时一并批（只在 Unix 上跑：程序是一段 shell 脚本，起来不握手，开关打开就算成）。
#[cfg(unix)]
#[tokio::test]
async fn pkg_approves_what_the_extension_asks_for_when_turning_it_on() {
    use std::os::unix::fs::PermissionsExt;

    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("approve");
    let dir = cwd.join("xcap");
    std::fs::create_dir_all(dir.join("bin")).expect("建得了目录");
    std::fs::write(
        dir.join("package.toml"),
        "[package]\nprotocol = [1, 1]\nname = { en = \"Cap\" }\n\n[command]\nname = \"xcap\"\nprogram = \"miyu-xcap\"\nabout = { en = \"Cap\" }\n\n[process]\nstart = \"manual\"\ncapabilities = [\"tools\"]\n",
    )
    .expect("写得进");
    let program = dir.join("bin/miyu-xcap");
    std::fs::write(&program, "#!/bin/sh\nsleep 30\n").expect("写得进");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("改得了权限");
    let installed = answering(&root, &cwd, &["pkg", "install", "./xcap"], "\n\n").await;
    let text = stdout(&installed);
    assert!(text.contains("\n需要："), "先列出要的能力：{text}");
    assert!(
        text.ends_with("装好了：xcap\n\n现在打开？[Y/n] 打开了：xcap\n"),
        "能力一并批了才打得开：{installed:?}"
    );
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}
