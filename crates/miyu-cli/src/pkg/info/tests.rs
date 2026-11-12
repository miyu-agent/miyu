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
        "from": "source", "source": "official", "package": "im", "version": "0.2.1",
        "name": "接入 QQ", "summary": "让她在 QQ 群和私聊里说话",
        "depends": [], "recommends": ["im-sponsor", "memes"],
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
            "来源        : github.com/miyu-agent/miyu-im（官方）",
            "编号        : im",
            "名称        : 接入 QQ",
            "版本        : 0.2.1",
            "描述        : 让她在 QQ 群和私聊里说话",
            "依赖        : 无",
            "推荐        : im-sponsor  memes",
            "提供        : 命令 miyu im、技能 sticker",
            "需要的能力  : 联网、替主人说话",
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
            "Origin      : github.com/someone/x (community, not officially reviewed)",
            "Name        : x",
            "Version     : 1",
            "Depends On  : None",
            "Recommends  : None",
        ],
        "没有值的行不印，依赖、推荐没有的写 None"
    );
}

/// 官方源里社区收录的包（施工 CM-1）：第一行「软件源」写的是源的名字 `official`，紧接着一行「来源」说清是社区的、没经过官方审核，
/// 不会被看成官方的包。
#[test]
fn a_community_package_in_the_official_source_says_so_right_under_the_source() {
    let info = json!({"from": "source", "source": "official", "package": "weather", "version": "0.3.0",
                      "origin": "github.com/ann/weather", "official": false});
    let shown = rows(&info, east_eight(), &Language::Chinese);
    assert_eq!(
        shown[..3],
        [
            "软件源  : official",
            "来源    : github.com/ann/weather（社区，没有经过官方审核）",
            "编号    : weather",
        ]
    );
    assert_eq!(
        rows(&info, east_eight(), &Language::English)[1],
        "Origin      : github.com/ann/weather (community, not officially reviewed)"
    );
    let mut official = info.clone();
    official["official"] = json!(true);
    assert_eq!(
        rows(&official, east_eight(), &Language::Chinese)[1],
        "来源    : github.com/ann/weather（官方）"
    );
}

#[test]
fn an_installed_package_has_who_needs_it_and_when_it_came() {
    let info = json!({
        "package": "pudding", "layer": "home", "from": "official", "origin": "github.com/a/pudding",
        "version": "2.0", "name": "Pudding", "files": 3, "size": 1536,
        "installed": "2026-10-11T06:03:00.000Z", "path": "/w/pudding.miyu", "source": "community",
        "feed": "official",
        "depends": ["base"], "required_by": ["a", "b"],
        "contains": {"program": "process", "command": "pudding", "page": true, "settings": 2},
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
            "Origin          : github.com/a/pudding (community, not officially reviewed)",
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

/// 来源照核心的 `source`（施工 PK-6）：不知道的照实写「不知道」，不说成官方；连的核心还没有 `source` 的照 `from` 只说「软件源」。
#[test]
fn an_installed_package_says_where_it_came_from_and_admits_when_it_does_not_know() {
    let origin_row = |info: serde_json::Value| {
        rows(&info, east_eight(), &Language::Chinese)
            .into_iter()
            .find(|row| row.starts_with("来源"))
            .expect("有来源这一行")
    };
    let home = |source: Option<&str>| {
        let mut info = json!({"package": "x", "layer": "home", "from": "official",
            "origin": "github.com/a/x", "files": 1, "size": 1});
        if let Some(source) = source {
            info["source"] = json!(source);
        }
        info
    };
    assert_eq!(
        origin_row(home(Some("unknown"))),
        "来源        : github.com/a/x（不知道）"
    );
    assert_eq!(
        origin_row(home(Some("official"))),
        "来源        : github.com/a/x（官方）"
    );
    assert_eq!(
        origin_row(home(None)),
        "来源        : github.com/a/x（软件源）",
        "老核心"
    );
    let distro = json!({"package": "x", "layer": "distro", "from": "distro", "source": "distro",
        "files": 1, "size": 1});
    assert_eq!(origin_row(distro), "来源        : 系统装的");
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

#[test]
fn a_package_folder_reads_like_an_installed_one_and_says_what_cannot_be_read() {
    let info = json!({
        "from": "file", "path": "/work/my-skill", "package": "my-skill", "version": "1.2",
        "name": "我的技能", "summary": "做点事", "author": "Ann",
        "contains": {"skills": ["notes"], "scripts": ["hello"]},
        "skipped": [{"file": "skills/bad/SKILL.md", "why": "missing front matter"},
                    {"file": "scripts/x.sh", "why": "not text"}],
        "files": 4, "size": 412, "replaces": {"version": "1.1"},
    });
    assert_eq!(
        rows(&info, east_eight(), &Language::Chinese),
        [
            "编号        : my-skill",
            "名称        : 我的技能",
            "版本        : 1.2",
            "描述        : 做点事",
            "依赖        : 无",
            "推荐        : 无",
            "提供        : 技能 notes、脚本 hello",
            "无法读取    : skills/bad/SKILL.md（missing front matter）、scripts/x.sh（not text）",
            "作者        : Ann",
            "安装后大小  : 412 B",
        ],
        "照装着的那几行，没有来源、安装时间；路径、几个文件、换下哪一份不印"
    );
    assert_eq!(
        rows(&info, east_eight(), &Language::English)[7],
        "Unreadable      : skills/bad/SKILL.md (missing front matter), scripts/x.sh (not text)"
    );
}
