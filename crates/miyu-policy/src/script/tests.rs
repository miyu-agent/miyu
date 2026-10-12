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
