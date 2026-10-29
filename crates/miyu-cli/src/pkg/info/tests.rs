//! `miyu pkg info` 印成的「键 : 值」（施工 IN-2，设计 `49-装包照pacman.md` 第一节）。

use serde_json::json;

use miyu_kernel::time::UtcOffset;

use super::{params, rows};
use crate::language::Language;

fn east_eight() -> UtcOffset {
    UtcOffset::from_minutes(8 * 60).expect("合法")
}

#[test]
fn a_package_from_the_source_is_keyed_rows_aligned_by_display_width() {
    let info = json!({
        "from": "source", "source": "official", "package": "miyu-im", "version": "0.2.1",
        "name": "接入 QQ", "summary": "让她在 QQ 群和私聊里说话",
        "depends": [], "recommends": ["miyu-im-sponsor", "miyu-memes"],
        "provides": {"command": ["im"], "features": ["im"], "skills": ["sticker"]},
        "capabilities": [{"id": "network", "name": "联网"}, {"id": "act_for_external", "name": "替主人说话"}],
        "origin": "github.com/miyu-agent/miyu-im", "official": true, "author": "miyu-agent",
        "upstream": "https://github.com/miyu-agent/miyu-im",
        "download": 2_306_867, "installed_size": 7_130_317,
    });
    assert_eq!(
        rows(&info, east_eight(), &Language::Chinese),
        [
            "软件源      : official",
            "编号        : miyu-im",
            "名称        : 接入 QQ",
            "版本        : 0.2.1",
            "描述        : 让她在 QQ 群和私聊里说话",
            "依赖        : 无",
            "推荐        : miyu-im-sponsor  miyu-memes",
            "提供        : 命令 miyu im、技能 sticker",
            "需要的能力  : 联网、替主人说话",
            "来源        : github.com/miyu-agent/miyu-im（官方）",
            "作者        : miyu-agent",
            "上游        : https://github.com/miyu-agent/miyu-im",
            "下载大小    : 2.2 MiB",
            "安装后大小  : 6.8 MiB",
        ],
        "照设计 49 的样子：中文一个字两格"
    );
    let community = json!({"from": "source", "source": "friends", "package": "x", "version": "1",
                           "origin": "github.com/someone/x", "official": false});
    assert_eq!(
        rows(&community, east_eight(), &Language::English),
        [
            "Repository  : friends",
            "Name        : x",
            "Version     : 1",
            "Depends On  : None",
            "Recommends  : None",
            "Origin      : github.com/someone/x (community)",
        ],
        "没有值的行不印，依赖、推荐没有的写 None"
    );
}

#[test]
fn an_installed_package_has_who_needs_it_and_when_it_came() {
    let info = json!({
        "package": "pudding", "layer": "home", "from": "official", "origin": "github.com/a/pudding",
        "version": "2.0", "name": "Pudding", "files": 3, "size": 1536,
        "installed": "2026-10-11T06:03:00.000Z", "source": "/w/pudding.miyu",
        "depends": ["base"], "required_by": ["a", "b"],
        "provides": {"program": "process", "command": "pudding", "page": true, "settings": 2},
        "capabilities": [{"id": "network", "name": "network"}],
    });
    assert_eq!(
        rows(&info, east_eight(), &Language::English),
        [
            "Name            : pudding",
            "Title           : Pudding",
            "Version         : 2.0",
            "Depends On      : base",
            "Recommends      : None",
            "Provides        : extension, command miyu pudding, admin page, 2 settings",
            "Capabilities    : network",
            "Origin          : github.com/a/pudding (repository)",
            "Installed Size  : 1.5 KiB",
            "Required By     : a  b",
            "Install Date    : 2026-10-11 14:03",
        ],
        "从哪装的路径、几个文件不印"
    );
    let shipped =
        json!({"package": "net", "layer": "shipped", "from": "shipped", "files": 1, "size": 10});
    assert_eq!(
        rows(&shipped, east_eight(), &Language::Chinese),
        [
            "编号        : net",
            "依赖        : 无",
            "推荐        : 无",
            "来源        : 出厂",
            "安装后大小  : 10 B",
            "安装时间    : 出厂自带",
        ]
    );
}

#[test]
fn si_and_qi_say_where_to_look() {
    assert_eq!(
        params("x", Some(true)),
        json!({"package": "x", "source": true})
    );
    assert_eq!(
        params("x", Some(false)),
        json!({"package": "x", "source": false})
    );
    assert_eq!(params("x", None), json!({"package": "x"}));
}
