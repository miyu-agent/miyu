//! 脚本的开头注释（施工 X-12 上）：旧版的写法读得出（全角、半角冒号，多行的参数，认、不用的键），没写参数的当没有参数；
//! 写错的说哪里不对；工具名照文件名。

use super::*;

const BANGUMI: &str = "#!/usr/bin/env python3
# Id: bangumi
# Display name: Bangumi
# 显示名称：番剧信息
# Description: Bangumi airing calendar: what airs today, search, details.
# 描述：查番组计划的放送日历与条目。
# Timeout: 120
# Group: research
# Parameters:
# {
#   \"type\": \"object\",
#   \"properties\": {
#     \"query\": { \"type\": \"string\", \"description\": \"Name: in any language.\" }
#   }
# }
# Permission: read-only

import sys
# 正文里的注释不算：Description: no
";

#[test]
fn the_old_header_reads() {
    let head = read("bangumi", BANGUMI).expect("读得出");
    assert_eq!(head.name, "bangumi");
    assert_eq!(
        head.description,
        "Bangumi airing calendar: what airs today, search, details."
    );
    assert_eq!(head.timeout, 120);
    assert_eq!(
        head.parameters,
        r#"{"properties":{"query":{"description":"Name: in any language.","type":"string"}},"type":"object"}"#
    );
    assert_eq!(
        head.display_name,
        BTreeMap::from([
            ("en".to_string(), "Bangumi".to_string()),
            ("zh".to_string(), "番剧信息".to_string()),
        ])
    );
    assert_eq!(
        head.summary,
        BTreeMap::from([("zh".to_string(), "查番组计划的放送日历与条目。".to_string())])
    );
}

#[test]
fn half_width_colons_slashes_and_no_parameters_read_too() {
    let text = "// 显示名称: 编解码\r\n// Description: Encode or decode.\r\nconsole.log(1)\r\n";
    let head = read("codec.js", text).expect("读得出");
    assert_eq!(head.name, "codec");
    assert_eq!(head.description, "Encode or decode.");
    assert_eq!(head.parameters, r#"{"type":"object","properties":{}}"#);
    assert_eq!(head.timeout, TIMEOUT);
    assert_eq!(
        head.display_name.get("zh").map(String::as_str),
        Some("编解码")
    );
    let one_line = "#!/bin/sh\n# Description: Hi.\n# Parameters: {\"type\":\"object\"}\necho\n";
    assert_eq!(
        read("hi.sh", one_line).map(|head| head.parameters),
        Ok(r#"{"type":"object"}"#.to_string())
    );
}

#[test]
fn file_names_become_tool_names() {
    assert_eq!(tool_name("get_weather.py").as_deref(), Some("get_weather"));
    assert_eq!(tool_name("douyin-dl").as_deref(), Some("douyin-dl"));
    assert_eq!(tool_name("番剧.v2.sh").as_deref(), Some("___v2"));
    assert_eq!(tool_name(".hidden"), None);
    assert_eq!(tool_name(".sh"), None);
    assert_eq!(
        tool_name(&format!("{}.py", "a".repeat(70))).map(|name| name.len()),
        Some(64)
    );
}

#[test]
fn wrong_ones_say_what_is_wrong() {
    let cases = [
        ("# 描述：只有给人看的。\n", ScriptProblem::NoDescription),
        ("#!/bin/sh\necho no header\n", ScriptProblem::NoDescription),
        (
            "# Description: Hi.\n# Timeout: soon\n",
            ScriptProblem::BadTimeout("soon".to_string()),
        ),
        (
            "# Description: Hi.\n# Timeout: 0\n",
            ScriptProblem::BadTimeout("0".to_string()),
        ),
        (
            "# Description: Hi.\n# Parameters: {\"type\": \"string\"}\n",
            ScriptProblem::NotAnObject,
        ),
    ];
    for (text, wanted) in cases {
        assert_eq!(read("a", text), Err(wanted), "{text}");
    }
    assert!(matches!(
        read("a", "# Description: Hi.\n# Parameters:\n# { \"type\": \n"),
        Err(ScriptProblem::BadParameters(_))
    ));
    assert!(matches!(
        read(
            "a",
            "# Description: Hi.\n# Parameters: {\"type\" \"object\"}\n"
        ),
        Err(ScriptProblem::BadParameters(_))
    ));
    assert_eq!(
        read(".a", "# Description: Hi.\n"),
        Err(ScriptProblem::NoName)
    );
}

/// 退役的 `Need load`（施工 L-1 登记、FL-1 拆掉）：以前写了的脚本照旧读得进，写什么都一样，不报错。
#[test]
fn a_retired_need_load_line_is_ignored() {
    let plain = read("a.py", "# Description: Hi.\n").expect("读得进");
    for line in [
        "# Need load: yes\n",
        "# Need load: no\n",
        "# Need load: maybe\n",
    ] {
        let head = read("a.py", &format!("# Description: Hi.\n{line}")).expect(line);
        assert_eq!(head, plain, "{line}");
    }
}

/// `Clicall`（施工 M-2）：`yes` 能在脚本里调，没写的、`no` 不能（施工 M-2 三补）；写错的报。
#[test]
fn clicall_is_yes_or_no() {
    for (line, on) in [
        ("# Clicall: no\n", false),
        ("# Clicall: Yes\n", true),
        ("", false),
    ] {
        let head = read("a.py", &format!("# Description: Hi.\n{line}")).expect(line);
        assert_eq!(head.clicall, on, "{line}");
    }
    let bad = read("a.py", "# Description: Hi.\n# Clicall: sometimes\n").unwrap_err();
    assert_eq!(bad, ScriptProblem::BadClicall("sometimes".to_string()));
    assert_eq!(bad.to_string(), "Clicall \"sometimes\" is not yes or no");
}

/// 脚本算的功能的编号（施工 X-12 中）：`script-<工具名>`。
#[test]
fn a_script_is_the_feature_script_dash_name() {
    assert_eq!(feature_id("get_weather"), "script-get_weather");
}

/// `Venues`（施工 X-43）：同清单的 `venues`，逗号隔开的几种，照写的先后；没写的没有（哪种会话都有）；空的、不认识的、重了的
/// 报 `BadVenues`；只有英文的键名，中文的 `场所` 不是键。
#[test]
fn venues_read_like_the_manifest() {
    for (line, venues) in [
        ("# Venues: local\n", vec!["local"]),
        ("# Venues: local, group\n", vec!["local", "group"]),
        (
            "# Venues：private,group ,local\n",
            vec!["private", "group", "local"],
        ),
    ] {
        let head = read("a.py", &format!("# Description: Hi.\n{line}")).expect(line);
        let venues: Vec<String> = venues.into_iter().map(str::to_string).collect();
        assert_eq!(head.venues, Some(venues), "{line}");
    }
    let head = read("a.py", "# Description: Hi.\n").expect("读得出");
    assert_eq!(head.venues, None, "没写的哪种会话都有");
    let head = read("a.py", "# Description: Hi.\n# 场所：local\n").expect("读得出");
    assert_eq!(head.venues, None, "中文的不是键名");
    for value in ["", "room", "local, local", "local,", "local group", "Local"] {
        let text = format!("# Description: Hi.\n# Venues: {value}\n");
        let bad = read("a.py", &text).unwrap_err();
        assert_eq!(
            bad,
            ScriptProblem::BadVenues(value.to_string()),
            "{value:?}"
        );
    }
    assert_eq!(
        ScriptProblem::BadVenues("room".to_string()).to_string(),
        "Venues \"room\" must list some of local, private, group, each once"
    );
}
