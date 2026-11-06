//! 装、卸之前印的那一份、问的那一句（施工 F-8 下补，`docs/blueprint/cli/pkg.md`）。

use serde_json::json;

use super::{asked, plan, question};
use crate::language::{Doing, Language};

#[test]
fn an_upgrade_says_what_it_replaces_carries_needs_and_weighs() {
    let shown = json!({
        "package": "pudding", "version": "1.2.0", "replaces": {"version": "1.1.0"},
        "program": "process", "command": "pudding", "page": true, "mascot": true,
        "connection": "qq", "settings": 2,
        "capabilities": [{"id": "network", "name": "联网", "summary": null},
                         {"id": "fs.read", "name": "读文件", "summary": null}],
        "files": 3, "size": 1536,
    });
    assert_eq!(
        plan(&shown, Doing::Install, &Language::Chinese),
        [
            "将安装 pudding 1.2.0（替换 1.1.0）",
            "包含：扩展程序、命令 miyu pudding、后台页、吉祥物、接入 qq、2 项设置",
            "需要：联网",
            "      读文件",
            "大小：1.5 KiB，3 个文件",
        ]
    );
    let bare = json!({"package": "x", "replaces": {"version": null}, "files": 1, "size": 9});
    assert_eq!(
        plan(&bare, Doing::Install, &Language::English),
        [
            "Will install x (replaces the installed one)",
            "Size: 9 B, 1 file"
        ],
        "没有的那一行不印"
    );
    // 只带 MCP 服务的（施工 X-2）：没有程序那一项，列出每个服务。
    let bundle = json!({"package": "gh", "mcp": ["github", "fetch"], "files": 1, "size": 9});
    assert_eq!(
        plan(&bundle, Doing::Install, &Language::Chinese),
        [
            "将安装 gh",
            "包含：MCP 服务 github、MCP 服务 fetch",
            "大小：9 B，1 个文件"
        ]
    );
    // 提供模型驱动的（施工 MD-1）：列出每个驱动，用它的模型时对话都经过它。
    let driver = json!({"package": "gravity", "program": "process", "drivers": ["ag"], "files": 1, "size": 9});
    assert_eq!(
        plan(&driver, Doing::Install, &Language::Chinese)[1],
        "包含：扩展程序、模型驱动 ag"
    );
    assert_eq!(
        plan(&driver, Doing::Install, &Language::English)[1],
        "Includes: extension, model driver ag"
    );
    let back = json!({"package": "net", "version": "1", "program": "builtin", "restores": true});
    assert_eq!(
        plan(&back, Doing::Restore, &Language::Chinese),
        ["将装回 net 1", "包含：内置功能"]
    );
}

#[test]
fn a_removal_says_what_goes_with_it() {
    let state = json!({"path": "/home/a/.miyu/var/lib/pudding", "size": 3_670_016});
    let config = json!({"path": "/home/a/.miyu/etc/pudding", "kept": true});
    let shown = json!({"package": "pudding", "layer": "home", "version": "1.2.0", "files": 3,
                       "size": 1536, "settings": ["pudding.port", "pudding.zones"], "state": state,
                       "config": config});
    assert_eq!(
        plan(&shown, Doing::Remove, &Language::Chinese),
        [
            "将卸载 pudding 1.2.0",
            "一并删除：设置 pudding.port、pudding.zones",
            "会删掉 /home/a/.miyu/var/lib/pudding（3.5 MiB）",
            "配置留在 /home/a/.miyu/etc/pudding",
            "大小：1.5 KiB，3 个文件",
        ]
    );
    let empty = json!({"package": "pudding", "layer": "shipped", "settings": [],
                       "state": {"path": "/x/var/lib/pudding", "size": 0},
                       "config": {"path": "/x/etc/pudding", "kept": true}});
    assert_eq!(
        plan(&empty, Doing::Remove, &Language::English),
        [
            "Will remove pudding",
            "Will delete /x/var/lib/pudding (0 B)",
            "Config kept in /x/etc/pudding"
        ]
    );
    // 写了 `--purge` 的（施工 PK-5）：配置目录也说会删掉。
    let purged = json!({"package": "pudding", "layer": "home", "settings": [],
                        "config": {"path": "/x/etc/pudding", "kept": false}});
    assert_eq!(
        plan(&purged, Doing::Remove, &Language::Chinese),
        ["将卸载 pudding", "会删掉 /x/etc/pudding"]
    );
    assert_eq!(
        plan(&purged, Doing::Remove, &Language::English),
        ["Will remove pudding", "Will delete /x/etc/pudding"]
    );
    // 旧的核心（施工 PK-4 以前）只说有没有：照旧列在「一并删除」里。
    let old = json!({"package": "pudding", "layer": "home", "settings": [], "state": true});
    assert_eq!(
        plan(&old, Doing::Remove, &Language::Chinese),
        ["将卸载 pudding", "一并删除：状态目录"]
    );
    let shipped = json!({"package": "net", "layer": "shipped", "settings": []});
    assert_eq!(
        plan(&shipped, Doing::Remove, &Language::English),
        ["Will remove net"]
    );
}

/// 配置里留下的设置项（施工 PK-5补）：留下的说几项，没有配置目录的单说设置；`--purge` 的键列在「一并删除」，不再说几项。
#[test]
fn a_removal_says_how_many_settings_stay() {
    let both = json!({"package": "pudding", "settings": [],
                      "config": {"path": "/x/etc/pudding", "kept": true, "settings": 2}});
    assert_eq!(
        plan(&both, Doing::Remove, &Language::Chinese),
        ["将卸载 pudding", "配置留在 /x/etc/pudding，另有 2 项设置"]
    );
    assert_eq!(
        plan(&both, Doing::Remove, &Language::English),
        [
            "Will remove pudding",
            "Config kept in /x/etc/pudding, plus 2 settings"
        ]
    );
    let only =
        json!({"package": "pudding", "settings": [], "config": {"kept": true, "settings": 1}});
    assert_eq!(
        plan(&only, Doing::Remove, &Language::Chinese),
        ["将卸载 pudding", "1 项设置留在配置里"]
    );
    assert_eq!(
        plan(&only, Doing::Remove, &Language::English),
        ["Will remove pudding", "1 setting kept in the config"]
    );
    let purged = json!({"package": "pudding", "settings": ["pudding.port"],
                        "config": {"kept": false, "settings": 1}});
    assert_eq!(
        plan(&purged, Doing::Remove, &Language::Chinese),
        ["将卸载 pudding", "一并删除：设置 pudding.port"]
    );
}

/// 答 `answer` 的结果、印在标准输出上的、印在标准错误上的。
fn answered(answer: &[u8], doing: Doing) -> (Result<(), u8>, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input = answer;
    let language = Language::Chinese;
    let question = language.proceed(doing, "app");
    let result = asked((&mut input, &mut out, &mut err), &question, &language);
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).expect("是 UTF-8");
    (result, text(out), text(err))
}

#[test]
fn enter_or_yes_goes_on_and_anything_else_or_nothing_stops() {
    for yes in [&b"\n"[..], b"y\n", b"Y\n", b"yes\n", b" YES \r\n"] {
        let (result, out, err) = answered(yes, Doing::Install);
        assert_eq!(result, Ok(()), "{yes:?}");
        assert_eq!((out.as_str(), err.as_str()), ("\n安装 'app'？[Y/n] ", ""));
    }
    for no in [&b"n\n"[..], b"no\n", b"x\n"] {
        let (result, _, err) = answered(no, Doing::Remove);
        assert_eq!((result, err.as_str()), (Err(1), "已取消\n"), "{no:?}");
    }
    let (result, out, err) = answered(b"", Doing::Remove);
    assert_eq!(result, Err(1), "标准输入关着的不做");
    assert_eq!(out, "\n卸载 'app'？[Y/n] \n");
    assert_eq!(
        err,
        "miyu: 已取消：没有收到确认\n提示：加 '--yes' 跳过确认\n"
    );
}

/// 只带技能、脚本的包（施工 X-13 上）：没有程序那一项，列出每个技能、脚本。
#[test]
fn skills_and_scripts_are_listed_by_name() {
    let content = json!({"package": "notes", "skills": ["take-notes"], "scripts": ["dice"], "slash": ["review"], "files": 3, "size": 9,
                         "skipped": [{"file": "skills/bad/SKILL.md", "why": "the head is not YAML"}]});
    assert_eq!(
        plan(&content, Doing::Install, &Language::Chinese),
        [
            "将安装 notes",
            "包含：技能 take-notes、脚本 dice、斜杠命令 /review",
            "无法读取：skills/bad/SKILL.md（the head is not YAML）",
            "大小：9 B，3 个文件"
        ]
    );
    assert_eq!(
        plan(&content, Doing::Install, &Language::English)[1],
        "Includes: skill take-notes, script dice, slash command /review"
    );
}

/// 从包文件夹装（施工 PK-3 下，设计 61 第三节的第三个样子）：来源写本机文件，能力一项一行带说明，核心给的「注意」接在问的前面，
/// 有要批的问「并批准」；装回出厂的写出厂、照旧那一句。
#[test]
fn a_local_folder_says_it_is_a_local_file_and_asks_to_approve() {
    let shown = |language: Language| {
        let (network, tools, notice) = match language {
            Language::Chinese => (
                ("联网", "访问网络、开端口"),
                ("提供工具", "给 AI 加能调用的工具"),
                [
                    "这个包没经过官方审核。",
                    "它带的程序会以你的身份在这台机器上跑，不在沙盒里。",
                ],
            ),
            Language::English => (
                ("Network", "Access the network and open ports"),
                ("Provide tools", "Add tools the AI can call"),
                [
                    "This package has not been officially reviewed.",
                    "The programs it carries run as you on this machine, outside any sandbox.",
                ],
            ),
        };
        json!({"package": "weather", "version": "0.3.0", "source": "file", "program": "process",
               "command": "weather", "settings": 2,
               "capabilities": [{"id": "network", "name": network.0, "summary": network.1},
                                {"id": "tools", "name": tools.0, "summary": tools.1}],
               "notice": notice, "files": 12, "size": 1536})
    };
    let zh = shown(Language::Chinese);
    assert_eq!(
        plan(&zh, Doing::Install, &Language::Chinese),
        [
            "将安装 weather 0.3.0",
            "包含：扩展程序、命令 miyu weather、2 项设置",
            "需要：联网：访问网络、开端口",
            "      提供工具：给 AI 加能调用的工具",
            "来源：本机文件",
            "大小：1.5 KiB，12 个文件",
        ]
    );
    assert_eq!(
        question(&zh, Doing::Install, "weather", &Language::Chinese),
        "注意：这个包没经过官方审核。它带的程序会以你的身份在这台机器上跑，不在沙盒里。\n\
         安装 'weather' 并批准它要的能力？[Y/n] "
    );
    let en = shown(Language::English);
    assert_eq!(
        plan(&en, Doing::Install, &Language::English),
        [
            "Will install weather 0.3.0",
            "Includes: extension, command miyu weather, 2 settings",
            "Needs: Network: Access the network and open ports",
            "       Provide tools: Add tools the AI can call",
            "From: local file",
            "Size: 1.5 KiB, 12 files",
        ]
    );
    assert_eq!(
        question(&en, Doing::Install, "weather", &Language::English),
        "Note: This package has not been officially reviewed. The programs it carries run as you on \
         this machine, outside any sandbox.\nInstall 'weather' and approve the capabilities it needs? [Y/n] "
    );
    let mut nothing = zh.clone();
    nothing["capabilities"] = json!([]);
    assert_eq!(
        question(&nothing, Doing::Install, "weather", &Language::Chinese),
        "注意：这个包没经过官方审核。它带的程序会以你的身份在这台机器上跑，不在沙盒里。\n\
         安装 'weather'？[Y/n] ",
        "没有要批的照旧那一句"
    );
    let mut older = zh.clone();
    for key in ["source", "notice"] {
        older.as_object_mut().map(|one| one.remove(key));
    }
    assert_eq!(
        question(&older, Doing::Install, "weather", &Language::Chinese),
        "安装 'weather'？[Y/n] ",
        "旧的核心不认 approve：不问批准"
    );
    let back = json!({"package": "net", "version": "1", "program": "builtin", "restores": true,
                      "source": "shipped", "capabilities": [{"id": "network", "name": "联网"}]});
    assert_eq!(
        plan(&back, Doing::Restore, &Language::Chinese),
        ["将装回 net 1", "包含：内置功能", "需要：联网", "来源：出厂"]
    );
    assert_eq!(
        question(&back, Doing::Restore, "net", &Language::Chinese),
        "装回 'net'？[Y/n] ",
        "出厂的算全批过"
    );
}
