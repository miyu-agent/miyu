//! 从软件源装之前印的那一份（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 2、3 条）：中英两份样子、没给的行不印、问的那一句。

use serde_json::{Value, json};

use super::{from_source, plan, question, recommends};
use crate::language::Language;

/// 设计 49 第二节那个样子：装 miyu-im，一起装 miyu-embed，推荐 miyu-im-sponsor。
fn preview() -> Value {
    json!({
        "package": "miyu-im", "version": "0.2.1", "official": true, "source": "official",
        "origin": "github.com/miyu-agent/miyu-im",
        "capabilities": [{"id": "network", "name": "联网"}, {"id": "act_for_external", "name": "替主人说话"}],
        "download": 12_897_485, "installed": 31_562_138,
        "with": [{"package": "miyu-embed", "version": "0.3.0"}],
        "recommends": [{"package": "miyu-im-sponsor", "version": "0.1.0", "download": 104_858}],
    })
}

#[test]
fn the_plan_reads_like_pacman_in_chinese() {
    assert_eq!(
        plan(&preview(), &Language::Chinese),
        [
            "软件包 (2)  miyu-embed-0.3.0  miyu-im-0.2.1",
            "推荐 (1)    miyu-im-sponsor-0.1.0",
            "",
            "下载大小    : 12.3 MiB",
            "安装后大小  : 30.1 MiB",
            "来源        : 官方，签名已验证",
            "需要的能力  : 联网、替主人说话",
        ]
    );
}

#[test]
fn the_plan_reads_like_pacman_in_english() {
    assert_eq!(
        plan(&preview(), &Language::English),
        [
            "Packages (2)     miyu-embed-0.3.0  miyu-im-0.2.1",
            "Recommended (1)  miyu-im-sponsor-0.1.0",
            "",
            "Download Size   : 12.3 MiB",
            "Installed Size  : 30.1 MiB",
            "Origin          : official, signature verified",
            "Capabilities    : 联网, 替主人说话",
        ]
    );
}

#[test]
fn rows_the_core_left_out_are_not_printed_and_other_sources_are_named() {
    let mut other = preview();
    for key in ["installed", "recommends", "capabilities", "with"] {
        other.as_object_mut().map(|object| object.remove(key));
    }
    other["official"] = json!(false);
    other["source"] = json!("friend");
    assert_eq!(
        plan(&other, &Language::Chinese),
        [
            "软件包 (1)  miyu-im-0.2.1",
            "",
            "下载大小  : 12.3 MiB",
            "来源      : friend，签名已验证，没人审过",
        ]
    );
    assert_eq!(
        plan(&other, &Language::English)[3],
        "Origin         : friend, signature verified, not reviewed"
    );
}

#[test]
fn one_package_is_asked_by_name_and_several_by_count() {
    let mut one = preview();
    one["with"] = json!([]);
    assert_eq!(question(&one, &Language::Chinese), "安装 'miyu-im'？[Y/n] ");
    assert_eq!(
        question(&preview(), &Language::Chinese),
        "安装 2 个软件包？[Y/n] "
    );
    assert_eq!(
        question(&preview(), &Language::English),
        "Install 2 packages? [Y/n] "
    );
}

#[test]
fn the_recommended_question_is_asked_only_when_there_are_some() {
    let listed = recommends(&preview());
    assert_eq!(
        listed,
        [("miyu-im-sponsor".to_string(), "0.1.0".to_string())]
    );
    assert_eq!(
        Language::Chinese.proceed_recommended(listed.len()),
        "同时安装推荐的 1 个软件包？[Y/n] "
    );
    assert_eq!(
        Language::English.proceed_recommended(2),
        "Also install 2 recommended packages? [Y/n] "
    );
    let mut none = preview();
    none["recommends"] = json!([]);
    assert!(recommends(&none).is_empty(), "没有推荐的不问");
    assert!(from_source(&preview()));
    assert!(
        !from_source(&json!({"package": "x", "files": 1, "size": 2})),
        "包目录的看一眼没有下载大小，照旧那一份"
    );
}
