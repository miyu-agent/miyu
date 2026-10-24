//! 真跑 `miyu pkg pack`（施工 G-1，设计 `37-软件源.md` 第二节第 4 条）：把包文件夹打成 `.miyu`，印文件名、SHA-256、大小，同一个
//! 文件夹再打一次哈希一样；打好的照 `install` 装得上。没写版本、`targets` 和 `--target` 对不上的不打，退出码 1。

use serde_json::Value;

use crate::support::cli::{run, stderr, stdout};
use crate::support::{Home, within};
use miyu_ipc::connect_or_start;

/// 印出来的路径和 `expected` 是不是同一个文件：照真实位置比（macOS 的临时目录在 `/private/var`，`/var` 是链接）。
fn same_file(printed: &str, expected: &std::path::Path) -> bool {
    let real = |path: &std::path::Path| std::fs::canonicalize(path).ok();
    real(std::path::Path::new(printed)).is_some()
        && real(std::path::Path::new(printed)) == real(expected)
}

/// 一份带版本、手动拉起的扩展包的清单（不会真的起），`more` 接在 `[package]` 里。
fn manifest(more: &str) -> String {
    format!(
        "[package]\nversion = \"1.0\"\nprotocol = [1, 1]\nname = {{ en = \"X\", zh = \"测试包\" }}\n{more}\n[command]\nname = \"xpkg\"\nprogram = \"miyu-nothing\"\nabout = {{ en = \"X\" }}\n\n[process]\nstart = \"manual\"\n"
    )
}

#[tokio::test]
async fn pkg_packs_a_folder_that_installs_again() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let cwd = home.dir.with_extension("pack");
    std::fs::create_dir_all(cwd.join("xpkg/bin")).expect("建得了目录");
    std::fs::write(cwd.join("xpkg/package.toml"), manifest("")).expect("写得进");
    std::fs::write(cwd.join("xpkg/bin/data.txt"), "1").expect("写得进");
    let zh = "zh_CN.UTF-8";

    let packed = run(&root, &cwd, zh, &["pkg", "pack", "./xpkg"]).await;
    assert_eq!(packed.status.code(), Some(0), "{packed:?}");
    let file = cwd.join("xpkg-1.0.miyu");
    let size = std::fs::metadata(&file).expect("打出来了").len();
    let text = stdout(&packed);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    let printed = lines[0].strip_prefix("打好了：").expect("第一行是路径");
    assert!(same_file(printed, &file), "{text}");
    let sha = lines[1].strip_prefix("SHA-256: ").expect("第二行是哈希");
    assert_eq!(sha.len(), 64, "{text}");
    assert_eq!(lines[2], format!("大小：{size} 字节"));

    let again = run(
        &root,
        &cwd,
        zh,
        &["pkg", "pack", "xpkg", "-o", "out", "--format", "json"],
    )
    .await;
    let shown: Value = serde_json::from_str(&stdout(&again)).expect("是 JSON");
    assert_eq!(shown["sha256"], sha, "同一个文件夹哈希一样：{shown}");
    assert_eq!(shown["package"], "xpkg");
    assert_eq!(shown["version"], "1.0");
    assert_eq!(shown["target"], Value::Null);
    let path = shown["path"].as_str().expect("有路径");
    assert!(
        same_file(path, &cwd.join("out").join("xpkg-1.0.miyu")),
        "{shown}"
    );

    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let installed = run(
        &root,
        &cwd,
        zh,
        &["pkg", "install", "xpkg-1.0.miyu", "--yes", "--system"],
    )
    .await;
    assert_eq!(stdout(&installed), "装好了：xpkg\n", "{installed:?}");
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

#[tokio::test]
async fn pkg_pack_refuses_without_a_version_or_a_fitting_target() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let cwd = home.dir.with_extension("nopack");
    std::fs::create_dir_all(cwd.join("xpkg")).expect("建得了目录");
    let en = "en_US.UTF-8";
    let cases = [
        (
            manifest("").replace("version = \"1.0\"\n", ""),
            vec![],
            "The manifest has no version ([package] version)",
        ),
        (
            manifest(""),
            vec!["--target", "linux-x86_64"],
            "The manifest has no targets: leave out --target",
        ),
        (
            manifest("targets = [\"linux-x86_64\", \"macos-aarch64\"]\n"),
            vec![],
            "This package is for linux-x86_64, macos-aarch64: pick one with --target",
        ),
        (
            manifest("targets = [\"linux-x86_64\"]\n"),
            vec!["--target", "windows-x86_64"],
            "--target windows-x86_64 is not in the manifest's targets (linux-x86_64)",
        ),
    ];
    for (text, extra, said) in cases {
        std::fs::write(cwd.join("xpkg/package.toml"), &text).expect("写得进");
        let mut args = vec!["pkg", "pack", "xpkg"];
        args.extend(extra);
        let refused = run(&root, &cwd, en, &args).await;
        assert_eq!(refused.status.code(), Some(1), "{refused:?}");
        assert_eq!(stderr(&refused), format!("{said}\n"));
    }
    std::fs::write(
        cwd.join("xpkg/package.toml"),
        manifest("targets = [\"linux-x86_64\"]\n"),
    )
    .expect("写得进");
    let fine = run(
        &root,
        &cwd,
        en,
        &["pkg", "pack", "xpkg", "--target", "linux-x86_64"],
    )
    .await;
    assert_eq!(fine.status.code(), Some(0), "{fine:?}");
    assert!(
        cwd.join("xpkg-1.0-linux-x86_64.miyu").is_file(),
        "文件名带系统"
    );
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 打包不碰数据根（施工 G-7）：作者的 CI 上没有数据根，打包模板也不该建一个；数据根那个位置放着别的东西也照打。
#[tokio::test]
async fn pkg_pack_leaves_the_data_root_alone() {
    let home = Home::new();
    let cwd = home.dir.with_extension("pack-alone");
    std::fs::create_dir_all(cwd.join("xpkg")).expect("建得了目录");
    std::fs::write(cwd.join("xpkg/package.toml"), manifest("")).expect("写得进");

    let missing = home.dir.with_extension("no-root");
    let packed = run(&missing, &cwd, "en_US.UTF-8", &["pkg", "pack", "xpkg"]).await;
    assert_eq!(packed.status.code(), Some(0), "{packed:?}");
    assert!(cwd.join("xpkg-1.0.miyu").exists());
    assert!(!missing.exists(), "没建数据根");

    let foreign = home.dir.with_extension("foreign-root");
    std::fs::create_dir_all(&foreign).expect("建得了目录");
    std::fs::write(foreign.join("notes.txt"), "not miyu").expect("写得进");
    let out = cwd.join("again");
    let again = run(
        &foreign,
        &cwd,
        "en_US.UTF-8",
        &["pkg", "pack", "xpkg", "-o", out.to_str().expect("UTF-8")],
    )
    .await;
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert!(out.join("xpkg-1.0.miyu").exists());
}
