//! 装了以后模型会多哪些工具（施工 PV-1）：脚本照开头读（和核心登记的同一份解析），显示名照两种语言；扩展照清单里功能写明的
//! 工具名；读不成的脚本、点开头的、子目录、链接不算；照名字排、去重；一件都没有的是空的。

use std::path::Path;

use super::*;
use crate::test_support::{Scratch, put};

fn manifest(text: &str) -> Manifest {
    miyu_config::package::read(text).expect("清单写对了")
}

const BUNDLE: &str = "[package]\nversion = \"1.0\"\nname = { en = \"Kit\" }\n";

fn titled(name: &str, title: &[(&str, &str)], from: &str) -> Tool {
    Tool {
        name: name.to_string(),
        title: title
            .iter()
            .map(|(lang, text)| ((*lang).to_string(), (*text).to_string()))
            .collect(),
        from: from.to_string(),
    }
}

#[test]
fn scripts_are_read_like_the_core_registers_them() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("kit");
    put(
        &folder,
        "scripts/crack_search.py",
        "#!/usr/bin/env python3\n# Description: Look up a game.\n# Display name: Game crack status\n# 显示名称: 游戏破解状态查询\nprint(1)\n".as_bytes(),
    );
    put(
        &folder,
        "scripts/plain.sh",
        b"#!/bin/sh\n# Description: Plain.\necho\n",
    );
    put(
        &folder,
        "scripts/only-zh.sh",
        "#!/bin/sh\n# Description: Z.\n# 显示名称: 只有中文\n".as_bytes(),
    );
    put(
        &folder,
        "scripts/broken.py",
        b"#!/usr/bin/env python3\nprint('no head')\n",
    );
    put(
        &folder,
        "scripts/.hidden.sh",
        b"#!/bin/sh\n# Description: Hidden.\n",
    );
    put(
        &folder,
        "scripts/nested/inner.sh",
        b"#!/bin/sh\n# Description: Inner.\n",
    );
    assert_eq!(
        of(&manifest(BUNDLE), &folder),
        [
            titled(
                "crack_search",
                &[("en", "Game crack status"), ("zh", "游戏破解状态查询")],
                "script"
            ),
            titled("only-zh", &[("zh", "只有中文")], "script"),
            titled("plain", &[], "script"),
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_linked_script_is_not_followed_out_of_the_package() {
    let scratch = Scratch::new();
    let outside = scratch.path().join("outside.sh");
    std::fs::write(&outside, "#!/bin/sh\n# Description: Outside.\n").expect("写得进");
    let folder = scratch.path().join("kit");
    put(
        &folder,
        "scripts/real.sh",
        b"#!/bin/sh\n# Description: Real.\n",
    );
    std::os::unix::fs::symlink(&outside, folder.join("scripts/linked.sh")).expect("造得了链接");
    let got = of(&manifest(BUNDLE), &folder);
    let names: Vec<&str> = got.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["real"]);
}

#[test]
fn an_extension_lists_the_tools_its_features_name() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("ext");
    std::fs::create_dir_all(&folder).expect("建得了");
    let text = "[package]\nversion = \"1.0\"\nprotocol = [1, 1]\nname = { en = \"Ext\" }\n\n[features.echo]\nname = { en = \"Echo\" }\ntools = [\"echo_twice\", \"echo_back\"]\n\n[features.more]\nname = { en = \"More\" }\ntools = [\"more\"]\n\n[command]\nname = \"ext\"\nprogram = \"miyu-ext\"\nabout = { en = \"Ext\" }\n\n[process]\n";
    assert_eq!(
        of(&manifest(text), &folder),
        [
            titled("echo_back", &[], "extension"),
            titled("echo_twice", &[], "extension"),
            titled("more", &[], "extension"),
        ]
    );
}

#[test]
fn a_package_without_tools_has_none() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("plain");
    std::fs::create_dir_all(&folder).expect("建得了");
    assert!(of(&manifest(BUNDLE), &folder).is_empty());
    assert!(of(&manifest(BUNDLE), Path::new("/nowhere/at/all")).is_empty());
}

#[test]
fn a_name_both_a_script_and_a_feature_bring_is_listed_once_as_the_script() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("both");
    put(
        &folder,
        "scripts/echo_back.sh",
        b"#!/bin/sh\n# Description: Echo.\n",
    );
    let text = "[package]\nversion = \"1.0\"\nprotocol = [1, 1]\nname = { en = \"Both\" }\n\n[features.echo]\nname = { en = \"Echo\" }\ntools = [\"echo_back\"]\n\n[command]\nname = \"both\"\nprogram = \"miyu-both\"\nabout = { en = \"Both\" }\n\n[process]\n";
    assert_eq!(
        of(&manifest(text), &folder),
        [titled("echo_back", &[], "script")]
    );
}
