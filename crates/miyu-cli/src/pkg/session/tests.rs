//! 会话里的 `miyu pkg` 换成哪几次调用（施工 BS-1 中下）。

use std::path::Path;

use clap::Parser;
use serde_json::{Value, json};

use super::calls;
use crate::pkg::Pkg;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    pkg: Pkg,
}

/// 照命令行 `miyu pkg <args>` 换，工作目录是 `/w`。
fn mapped(args: &[&str]) -> Result<Vec<(&'static str, Value)>, String> {
    let cli = Cli::try_parse_from(std::iter::once("pkg").chain(args.iter().copied()))
        .unwrap_or_else(|error| panic!("{args:?}：{error}"));
    calls(cli.pkg.command, Path::new("/w"))
}

#[test]
fn reading_goes_to_packages() {
    assert_eq!(mapped(&[]), Ok(vec![("packages", json!({}))]));
    assert_eq!(mapped(&["list"]), Ok(vec![("packages", json!({}))]));
    assert_eq!(mapped(&["-Q"]), Ok(vec![("packages", json!({}))]));
    assert_eq!(
        mapped(&["info", "shell"]),
        Ok(vec![("packages", json!({"package": "shell"}))])
    );
    assert_eq!(
        mapped(&["-Qi", "shell"]),
        Ok(vec![("packages", json!({"package": "shell"}))])
    );
    let folder = Path::new("/w").join("kit").to_string_lossy().into_owned();
    assert_eq!(
        mapped(&["info", "./kit"]),
        Ok(vec![("packages", json!({"path": folder}))]),
        "写路径的看那个包文件夹"
    );
    assert_eq!(
        mapped(&["search", "web", "fetch"]),
        Ok(vec![("packages", json!({"search": "web fetch"}))])
    );
    assert_eq!(
        mapped(&["-Ss", "web"]),
        Ok(vec![("packages", json!({"search": "web"}))])
    );
}

#[test]
fn changing_goes_to_package_and_paths_become_folders() {
    let folder = Path::new("/w").join("kit").to_string_lossy().into_owned();
    assert_eq!(
        mapped(&["install", "kit/package.toml"]),
        Ok(vec![("package", json!({"install": folder}))]),
        "清单的路径取它所在的文件夹，相对的照工作目录"
    );
    assert_eq!(
        mapped(&["install", "."]),
        Ok(vec![(
            "package",
            json!({"install": Path::new("/w").to_string_lossy()})
        )]),
        "`.` 是现在的目录"
    );
    let file = Path::new("/w")
        .join("kit-1.miyu")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        mapped(&["install", "sub/../kit-1.miyu"]),
        Ok(vec![("package", json!({"install": file}))]),
        "包文件照原样，`..` 照字面去掉"
    );
    assert_eq!(
        mapped(&["install", "net"]),
        Ok(vec![("package", json!({"install": "net"}))])
    );
    assert_eq!(
        mapped(&["-S", "a", "b"]),
        Ok(vec![
            ("package", json!({"install": "a"})),
            ("package", json!({"install": "b"}))
        ])
    );
    assert_eq!(
        mapped(&["remove", "net", "--yes"]),
        Ok(vec![("package", json!({"remove": "net"}))]),
        "--yes 不认：问不问由权限策略判"
    );
    assert_eq!(
        mapped(&["upgrade"]),
        Ok(vec![("package", json!({"upgrade": "all"}))])
    );
    assert_eq!(
        mapped(&["-Syu"]),
        Ok(vec![("package", json!({"upgrade": "all"}))])
    );
}

#[test]
fn what_works_only_on_this_machine_is_refused() {
    for (args, said) in [
        (&["files", "shell"][..], "files"),
        (&["owns", "/x"], "owns"),
        (&["check"], "check"),
        (&["update"], "update"),
        (&["clean"], "clean"),
        (&["-Sy"], "-Sy"),
        (&["-Sc"], "-Sc"),
    ] {
        assert_eq!(mapped(args), Err(said.to_string()), "{args:?}");
    }
}
