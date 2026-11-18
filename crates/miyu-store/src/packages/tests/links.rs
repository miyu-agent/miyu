//! 装包时包文件夹里的链接（施工 AU-008，审计 #008 第 1 条）：照它指的拷，指到数据根里（包文件夹自己以外）的整个装不上，不把
//! 数据根里的东西拷进 `opt/`。

use std::fs;
use std::path::Path;

use crate::packages::install;
use crate::test_support::{Scratch, link};

/// 临时目录里：数据根 `data/`（里面有密钥文件、`opt/`），数据根外面的包文件夹 `src/`。
fn site() -> (Scratch, std::path::PathBuf, std::path::PathBuf) {
    let scratch = Scratch::new();
    let root = scratch.path().join("data");
    fs::create_dir_all(root.join("etc/private")).unwrap();
    fs::write(
        root.join("etc/private/secrets.toml"),
        "key = \"sk-secret\"\n",
    )
    .unwrap();
    let source = scratch.path().join("src");
    fs::create_dir_all(source.join("skills/s")).unwrap();
    fs::write(source.join("package.toml"), "x").unwrap();
    (scratch, root, source)
}

fn opt(root: &Path) -> std::path::PathBuf {
    root.join("opt")
}

/// 修以前：指到数据根里密钥文件的链接照内容拷成 `opt/<编号>/` 里的普通文件，模型、关起来的会话照技能文件夹读得到。
#[test]
fn a_link_into_the_data_root_is_not_copied() {
    let (_scratch, root, source) = site();
    if !link(
        &root.join("etc/private/secrets.toml"),
        &source.join("skills/s/secret.txt"),
    ) {
        return;
    }
    assert!(install::place(&opt(&root), "p", &source, &root).is_err());
    assert!(!opt(&root).join("p").exists());
    let leftovers: Vec<_> = fs::read_dir(opt(&root)).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "暂存删掉了：{leftovers:?}");
}

/// 指到包住数据根的目录（比如家目录）的：走下去会进数据根，同样装不上。
#[test]
fn a_link_to_a_folder_holding_the_data_root_is_not_copied() {
    let (scratch, root, source) = site();
    if !link(scratch.path(), &source.join("skills/s/home")) {
        return;
    }
    assert!(install::place(&opt(&root), "p", &source, &root).is_err());
    assert!(!opt(&root).join("p").exists());
}

/// 照旧：指到数据根外面的照内容拷；包文件夹在数据根里时，指到包文件夹自己里面的也照拷。
#[test]
fn links_outside_the_data_root_or_inside_the_package_still_copy() {
    let (scratch, root, _) = site();
    let outside = scratch.path().join("tool");
    fs::write(&outside, "#!/bin/sh\n").unwrap();
    // 包文件夹在数据根的工作区里（自己写包、照文件夹装的样子）。
    let source = root.join("workspace/mypkg");
    fs::create_dir_all(source.join("bin")).unwrap();
    fs::write(source.join("bin/real"), "real").unwrap();
    if !(link(&outside, &source.join("bin/tool"))
        && link(&source.join("bin/real"), &source.join("bin/alias")))
    {
        return;
    }
    let placed = install::place(&opt(&root), "p", &source, &root).expect("装得上");
    placed.keep();
    let installed = opt(&root).join("p/bin");
    assert_eq!(
        fs::read_to_string(installed.join("tool")).unwrap(),
        "#!/bin/sh\n"
    );
    assert_eq!(fs::read_to_string(installed.join("alias")).unwrap(), "real");
    assert!(
        !fs::symlink_metadata(installed.join("alias"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
