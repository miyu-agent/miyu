//! 从软件源装之前印的那一份（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 2、3 条）：中英两份样子、没给的行不印、问的那一句。
//! 施工 PK-3 下（设计 `61-装包说清来源和能力.md` 第三节）：来源照每个包说、能力一项一行带说明、「注意」、问的那一句带批准、推荐
//! 的另一段；旧的核心照旧。

use serde_json::{Value, json};

use super::{from_source, plan, question, recommended_question, recommends};
use crate::language::Language;

/// 设计 49 第二节那个样子：装 im，一起装 embed，推荐 im-sponsor；都是官方的。
fn preview() -> Value {
    json!({
        "package": "im", "version": "0.2.1", "official": true, "source": "official", "feed": "official",
        "origin": "github.com/miyu-agent/miyu-im",
        "capabilities": [{"id": "network", "name": "联网"}, {"id": "act_for_external", "name": "替主人说话"}],
        "download": 12_897_485, "installed": 31_562_138,
        "with": [{"package": "embed", "version": "0.3.0", "source": "official", "feed": "official"}],
        "recommends": [{"package": "im-sponsor", "version": "0.1.0", "download": 104_858,
                        "source": "official", "feed": "official"}],
    })
}

/// 能力一项：编号、名字、说明照 `language` 那一份核心的字。
fn capability(id: &str, language: &Language) -> Value {
    let (name, summary) = match (id, language) {
        ("network", Language::Chinese) => ("联网", "访问网络、开端口"),
        ("tools", Language::Chinese) => ("提供工具", "给 AI 加能调用的工具"),
        ("network", Language::English) => ("Network", "Access the network and open ports"),
        (_, Language::English) => ("Provide tools", "Add tools the AI can call"),
        _ => unreachable!(),
    };
    json!({"id": id, "name": name, "summary": summary})
}

/// 设计 61 第三节的第一个样子：社区的 weather，一个包，要联网、提供工具，带程序。
fn weather(language: &Language) -> Value {
    let notice = match language {
        Language::Chinese => json!([
            "这个包没经过官方审核。",
            "它带的程序会以你的身份在这台机器上跑，不在沙盒里。"
        ]),
        Language::English => json!([
            "This package has not been officially reviewed.",
            "The programs it carries run as you on this machine, outside any sandbox."
        ]),
    };
    json!({
        "package": "weather", "version": "0.3.0", "official": false, "source": "community", "feed": "official",
        "origin": "github.com/ann/weather",
        "capabilities": [capability("network", language), capability("tools", language)],
        "download": 1_258_291, "installed": 3_565_158, "with": [], "recommends": [],
        "notice": notice,
    })
}

/// 设计 61 第三节的第二个样子：再一起装官方的 onebot（要联网）。
fn weather_with_onebot(language: &Language) -> Value {
    let mut shown = weather(language);
    shown["with"] = json!([{"package": "onebot", "version": "1.2.0", "source": "official",
                            "feed": "official", "capabilities": [capability("network", language)]}]);
    shown["download"] = json!(1_992_295);
    shown["installed"] = json!(5_242_880);
    shown["notice"] = match language {
        Language::Chinese => json!([
            "weather 没经过官方审核。",
            "weather 带的程序会以你的身份在这台机器上跑，不在沙盒里。"
        ]),
        Language::English => json!([
            "Not officially reviewed: weather.",
            "The programs carried by weather run as you on this machine, outside any sandbox."
        ]),
    };
    shown
}

#[test]
fn a_community_package_says_where_it_comes_from_what_it_needs_and_asks_to_approve() {
    let zh = weather(&Language::Chinese);
    assert_eq!(
        plan(&zh, &Language::Chinese),
        [
            "软件包 (1)  weather-0.3.0",
            "",
            "下载大小    : 1.2 MiB",
            "安装后大小  : 3.4 MiB",
            "来源        : 社区，签名已验证（github.com/ann/weather）",
            "需要的能力  : 联网：访问网络、开端口",
            "              提供工具：给 AI 加能调用的工具",
        ]
    );
    assert_eq!(
        question(&zh, &Language::Chinese),
        "注意：这个包没经过官方审核。它带的程序会以你的身份在这台机器上跑，不在沙盒里。\n\
         安装 'weather' 并批准它要的能力？[Y/n] "
    );
    let en = weather(&Language::English);
    assert_eq!(
        plan(&en, &Language::English),
        [
            "Packages (1)  weather-0.3.0",
            "",
            "Download Size   : 1.2 MiB",
            "Installed Size  : 3.4 MiB",
            "Origin          : community, signature verified (github.com/ann/weather)",
            "Capabilities    : Network: Access the network and open ports",
            "                  Provide tools: Add tools the AI can call",
        ]
    );
    assert_eq!(
        question(&en, &Language::English),
        "Note: This package has not been officially reviewed. The programs it carries run as you on \
         this machine, outside any sandbox.\nInstall 'weather' and approve the capabilities it needs? [Y/n] "
    );
}

#[test]
fn several_packages_from_different_places_are_named_on_each_line() {
    let zh = weather_with_onebot(&Language::Chinese);
    assert_eq!(
        plan(&zh, &Language::Chinese),
        [
            "软件包 (2)  onebot-1.2.0  weather-0.3.0",
            "",
            "下载大小    : 1.9 MiB",
            "安装后大小  : 5.0 MiB",
            "来源        : onebot   官方，签名已验证",
            "              weather  社区，签名已验证（github.com/ann/weather）",
            "需要的能力  : onebot   联网：访问网络、开端口",
            "              weather  联网：访问网络、开端口",
            "              weather  提供工具：给 AI 加能调用的工具",
        ]
    );
    assert_eq!(
        question(&zh, &Language::Chinese),
        "注意：weather 没经过官方审核。weather 带的程序会以你的身份在这台机器上跑，不在沙盒里。\n\
         安装 2 个软件包并批准它们要的能力？[Y/n] "
    );
    let en = weather_with_onebot(&Language::English);
    assert_eq!(
        plan(&en, &Language::English)[4..],
        [
            "Origin          : onebot   official, signature verified",
            "                  weather  community, signature verified (github.com/ann/weather)",
            "Capabilities    : onebot   Network: Access the network and open ports",
            "                  weather  Network: Access the network and open ports",
            "                  weather  Provide tools: Add tools the AI can call",
        ]
    );
    assert_eq!(
        question(&en, &Language::English),
        "Note: Not officially reviewed: weather. The programs carried by weather run as you on this \
         machine, outside any sandbox.\nInstall 2 packages and approve the capabilities they need? [Y/n] "
    );
}

#[test]
fn official_packages_have_no_notice_and_one_origin_line() {
    assert_eq!(
        plan(&preview(), &Language::Chinese),
        [
            "软件包 (2)  embed-0.3.0  im-0.2.1",
            "推荐 (1)    im-sponsor-0.1.0",
            "",
            "下载大小    : 12.3 MiB",
            "安装后大小  : 30.1 MiB",
            "来源        : 官方，签名已验证",
            "需要的能力  : im     联网",
            "              im     替主人说话",
        ]
    );
    assert_eq!(
        plan(&preview(), &Language::English)[3..],
        [
            "Download Size   : 12.3 MiB",
            "Installed Size  : 30.1 MiB",
            "Origin          : official, signature verified",
            "Capabilities    : im     联网",
            "                  im     替主人说话",
        ]
    );
    assert_eq!(
        question(&preview(), &Language::Chinese),
        "安装 2 个软件包并批准它们要的能力？[Y/n] ",
        "官方的没有「注意」"
    );
}

#[test]
fn the_question_asks_to_approve_only_what_is_not_held_yet() {
    let mut held = weather(&Language::Chinese);
    held["notice"] = Value::Null;
    held["source"] = json!("official");
    for one in held["capabilities"].as_array_mut().into_iter().flatten() {
        one["held"] = json!(true);
    }
    assert_eq!(
        question(&held, &Language::Chinese),
        "安装 'weather'？[Y/n] "
    );
    assert_eq!(
        plan(&held, &Language::Chinese)[4..],
        [
            "来源        : 官方，签名已验证",
            "需要的能力  : 联网：访问网络、开端口（已批准）",
            "              提供工具：给 AI 加能调用的工具（已批准）",
        ]
    );
    held["capabilities"][1]
        .as_object_mut()
        .map(|one| one.remove("held"));
    assert_eq!(
        question(&held, &Language::English),
        "Install 'weather' and approve the capabilities it needs? [Y/n] "
    );
    let mut none = held.clone();
    none["capabilities"] = json!([]);
    assert_eq!(
        question(&none, &Language::Chinese),
        "安装 'weather'？[Y/n] "
    );
}

#[test]
fn an_older_core_is_printed_and_asked_as_before() {
    let mut other = preview();
    for key in ["installed", "recommends", "with", "feed"] {
        other.as_object_mut().map(|object| object.remove(key));
    }
    other["official"] = json!(false);
    other["source"] = json!("friend");
    assert_eq!(
        plan(&other, &Language::Chinese),
        [
            "软件包 (1)  im-0.2.1",
            "",
            "下载大小    : 12.3 MiB",
            "来源        : friend，签名已验证，没人审过",
            "需要的能力  : 联网",
            "              替主人说话",
        ]
    );
    assert_eq!(
        plan(&other, &Language::English)[3],
        "Origin         : friend, signature verified, not reviewed"
    );
    assert_eq!(
        question(&other, &Language::Chinese),
        "安装 'im'？[Y/n] ",
        "不认 approve 的核心不问批准"
    );
    let mut friend = weather(&Language::Chinese);
    friend["feed"] = json!("friends");
    assert_eq!(
        plan(&friend, &Language::Chinese)[4],
        "来源        : friends，签名已验证（github.com/ann/weather）",
        "别人加的源写源的名字"
    );
}

#[test]
fn recommended_packages_that_need_capabilities_get_their_own_part() {
    let mut shown = weather(&Language::Chinese);
    shown["recommends"] = json!([{"package": "rec", "version": "1.0", "source": "community",
                                  "feed": "official", "capabilities": [capability("network", &Language::Chinese)]},
                                 {"package": "plain", "version": "1.0", "source": "official", "feed": "official"}]);
    shown["recommends_notice"] = json!(["rec 没经过官方审核。"]);
    assert_eq!(
        recommended_question(&shown, &Language::Chinese),
        "\n推荐的包要的能力  : rec    联网：访问网络、开端口\n注意：rec 没经过官方审核。\n\
         同时安装推荐的 2 个软件包并批准它们要的能力？[Y/n] "
    );
    let mut english = weather(&Language::English);
    english["recommends"] = json!([{"package": "rec", "version": "1.0", "source": "official",
                                    "feed": "official", "capabilities": [capability("tools", &Language::English)]}]);
    assert_eq!(
        recommended_question(&english, &Language::English),
        "\nRecommended Capabilities  : rec  Provide tools: Add tools the AI can call\n\
         Also install 1 recommended package and approve the capabilities it needs? [Y/n] "
    );
}

#[test]
fn the_recommended_question_is_asked_only_when_there_are_some() {
    let listed = recommends(&preview());
    assert_eq!(listed, [("im-sponsor".to_string(), "0.1.0".to_string())]);
    assert_eq!(
        recommended_question(&preview(), &Language::Chinese),
        "同时安装推荐的 1 个软件包？[Y/n] ",
        "推荐的不要能力、官方的照旧那一句"
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
