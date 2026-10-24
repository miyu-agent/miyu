//! 软件包加的子命令（施工 9-2，`docs/blueprint/packages.md`「转交」）：真二进制走一遍。管理员家目录里放一份清单，子命令
//! `probe` 跑的程序就是 `miyu` 自己（它在 `miyu` 旁边）：参数原样交过去、退出码照它的；`miyu help probe` 转成 `--help`；
//! `miyu -h` 多一节，撞了内置的不列；没装的程序说没装、退出码 1；不认识的照旧退出码 2，不拉起核心。

use std::process::{Command, Output};

use crate::support::{Home, MIYU};

/// 一份清单：子命令 `name` 跑 `program`。
fn manifest(name: &str, program: &str, about: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"P\" }}\n\n[command]\nname = \"{name}\"\nprogram = \"{program}\"\nabout = {{ en = \"{about}\", zh = \"{about}（中）\" }}\n\n[ui]\n"
    )
}

fn install(home: &Home, id: &str, text: &str) {
    let dir = home.root.path().join("system/packages").join(id);
    std::fs::create_dir_all(&dir).expect("建得了目录");
    std::fs::write(dir.join("package.toml"), text).expect("写得进");
}

/// 在临时的数据根上跑 `miyu <args>`，界面语言是 `lang`。
fn miyu(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .env("MIYU_RESOURCES", crate::support::resources())
        .envs(crate::support::offline(home.root.path()))
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_package_command_runs_its_program_with_the_same_arguments() {
    let home = Home::new();
    install(&home, "probe", &manifest("probe", "miyu", "Probe it"));
    let version = miyu(&home, "C", &["probe", "--version"]);
    assert_eq!(version.status.code(), Some(0), "{}", text(&version.stderr));
    assert!(
        text(&version.stdout).starts_with("miyu "),
        "参数原样交过去：{}",
        text(&version.stdout)
    );
    let wrong = miyu(&home, "C", &["probe", "nonsense"]);
    assert_eq!(wrong.status.code(), Some(2), "退出码照它的");
    assert!(
        text(&wrong.stderr).contains("nonsense"),
        "{}",
        text(&wrong.stderr)
    );
    let help = miyu(&home, "C", &["help", "probe"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(
        text(&help.stdout).starts_with("Usage: miyu"),
        "帮助由它自己说：{}",
        text(&help.stdout)
    );
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty(), "核心没起来过");
}

/// 包自己带程序（施工 F-8 上再补）：放在包文件夹的 `bin/` 里的，`miyu` 旁边没有也照它跑。这里放的是指向 `miyu` 的链接
/// （Windows 上拷一份），名字只有这个包用。
#[test]
fn a_package_command_runs_the_program_in_its_bin() {
    let home = Home::new();
    let program = format!("miyu-own-{}", std::process::id());
    install(&home, "own", &manifest("own", &program, "Own it"));
    let bin = home.root.path().join("system/packages/own/bin");
    std::fs::create_dir_all(&bin).expect("建得了目录");
    let target = bin.join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
    #[cfg(unix)]
    std::os::unix::fs::symlink(MIYU, &target).expect("链得上");
    #[cfg(not(unix))]
    std::fs::copy(MIYU, &target).expect("拷得了");
    let version = miyu(&home, "C", &["own", "--version"]);
    assert_eq!(version.status.code(), Some(0), "{}", text(&version.stderr));
    assert!(
        text(&version.stdout).starts_with("miyu "),
        "跑的是包里的那一个：{}",
        text(&version.stdout)
    );
}

/// 转交时环境多 `MIYU_PACKAGE_DIR`，是这个包的文件夹（施工 S-6 上），和 `MIYU_PACKAGE_STATE`，状态目录（施工 F-15）：包里带的程序是指向系统的 `printenv` 的链接，印出它
/// 看到的值。只在 Unix 上测（Windows 上没有现成的 `printenv`；设环境变量那一步由 miyu-cli 的单元测试守着，两个平台一条路）。
#[cfg(unix)]
#[test]
fn a_package_command_is_told_its_package_folder() {
    let home = Home::new();
    let program = format!("miyu-printenv-{}", std::process::id());
    install(&home, "own", &manifest("own", &program, "Own it"));
    let dir = home.root.path().join("system/packages/own");
    std::fs::create_dir_all(dir.join("bin")).expect("建得了目录");
    std::os::unix::fs::symlink("/usr/bin/printenv", dir.join("bin").join(&program))
        .expect("链得上");
    let printed = miyu(&home, "C", &["own", "MIYU_PACKAGE_DIR"]);
    assert_eq!(printed.status.code(), Some(0), "{}", text(&printed.stderr));
    assert_eq!(text(&printed.stdout), format!("{}\n", dir.display()));
    // 状态目录（施工 F-15）：跑之前建好，交真实的位置。
    let printed = miyu(&home, "C", &["own", "MIYU_PACKAGE_STATE"]);
    assert_eq!(printed.status.code(), Some(0), "{}", text(&printed.stderr));
    let state =
        std::fs::canonicalize(home.root.path().join("state/packages/own")).expect("跑之前建好了");
    assert_eq!(text(&printed.stdout), format!("{}\n", state.display()));
}

#[test]
fn the_help_page_lists_package_commands_but_not_ones_taken_by_builtins() {
    let home = Home::new();
    install(&home, "probe", &manifest("probe", "miyu", "Probe it"));
    install(&home, "taken", &manifest("ask", "miyu", "Not me"));
    let zh = text(&miyu(&home, "zh_CN.UTF-8", &["-h"]).stdout);
    assert!(
        zh.contains("\n\n软件包加的命令：\n  probe                 Probe it（中）\n\n"),
        "{zh}"
    );
    let en = text(&miyu(&home, "C", &["--help"]).stdout);
    assert!(
        en.contains("\n\nCommands from packages:\n  probe                 Probe it\n\n"),
        "{en}"
    );
    assert!(!en.contains("Not me"), "撞了内置的不列：{en}");
    // 内置的优先：`miyu ask` 照旧是内置的那个。
    let ask = miyu(&home, "C", &["ask", "--help"]);
    assert!(
        text(&ask.stdout).starts_with("Usage: miyu ask"),
        "{}",
        text(&ask.stdout)
    );
    // 只有出厂的：出厂的包都不带命令（两个界面的清单施工 S-5、接入QQ 施工 S-6 下搬出了核心仓库），一个都没有的不写这一节。
    let plain = Home::new();
    let shipped = text(&miyu(&plain, "C", &["-h"]).stdout);
    assert!(!shipped.contains("Commands from packages"), "{shipped}");
}

#[test]
fn a_missing_program_is_said_and_an_unknown_word_is_still_refused() {
    let home = Home::new();
    install(
        &home,
        "ghost",
        &manifest("ghost", "miyu-no-such-program-anywhere", "Ghost"),
    );
    let missing = miyu(&home, "C", &["ghost"]);
    assert_eq!(missing.status.code(), Some(1));
    let said = text(&missing.stderr);
    assert!(
        said.starts_with("miyu-no-such-program-anywhere not found: ")
            && said.contains(&format!(
                "{} says miyu ghost runs it",
                std::path::Path::new("ghost").join("package.toml").display()
            )),
        "{said}"
    );
    let unknown = miyu(&home, "C", &["hello"]);
    assert_eq!(unknown.status.code(), Some(2), "不认识的照旧拒");
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
}

/// 清单是内置包、这一份核心没编进它的代码（施工 F-2，设计 30 第二节第 3 条）：真核心起来时照读坏了的清单记一行
/// `WARN package invalid`，带 `not_built_in`；编进来了的出厂内置包不记。
#[tokio::test]
async fn the_core_says_which_builtin_it_lacks() {
    let home = Home::new();
    install(
        &home,
        "xghost",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Ghost\" }\n\n[builtin]\n",
    );
    let (connection, token) = crate::support::within(
        "拉起",
        miyu_ipc::connect_or_start(&home.root, || home.core()),
    )
    .await
    .expect("拉得起");
    let reply = crate::support::hello(connection, &token).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_stopped().await;
    let log = home.core_log();
    let lacking: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("package invalid"))
        .collect();
    assert_eq!(lacking.len(), 1, "{log}");
    assert!(lacking[0].contains("xghost"), "{log}");
    assert!(lacking[0].contains("built-in package xghost"), "{log}");
    assert_eq!(
        crate::support::count(&log, "required package missing"),
        0,
        "{log}"
    );
}
