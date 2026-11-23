//! 扮扩展、扮平台桥的两份剧本（施工 LW-30，写法见 `heavy/role.rs` 开头）。两份都是先做起来时的那几步（登记工具、方法，
//! 往 system 尾部放一段，登记端口，读配置），再停在 `pause` 等量尺放行；放行以后先 `once`，重启以后起来的不再做后面的。

use serde_json::{Value, json};

/// 扩展登记的两件工具：查找（只读，群里、私聊里也给，脚本里能调）、记一笔（会改东西）。
pub fn tools() -> Value {
    json!([
        {"name": "perf_lookup", "description": "Look up an entry.",
         "input_schema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]},
         "access": "read", "venues": ["local", "private", "group"], "display_name": {"en": "Lookup", "zh": "查找"},
         "subject": "query", "clicall": true, "result": "{\"items\":[{\"id\"}]}"},
        {"name": "perf_note", "description": "Write a note.",
         "input_schema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]},
         "access": "write", "venues": ["local"], "display_name": {"en": "Note", "zh": "记一笔"}},
    ])
}

/// 扩展（`perf-tools`）的剧本：放行以后交预设功能，自己开会话说一句（带 [`super::steps::EXTENSION_SAYS`]）、等这一轮结束，
/// 读读写写会话（列表、视图、工具面、事件、状态）、界面块、记忆、知识库、模型、人格、技能、总线、包带的内容、给人看的字，
/// 放开端口、撤掉放在 system 尾部的那一段。
pub fn extension() -> Value {
    let call = |method: &str, params: Value| json!({"call": method, "params": params});
    json!([
        call("provide", json!({"tools": tools()})),
        call("package.methods", json!({"methods": [{"name": "status"}, {"name": "refresh"}]})),
        call("system.append", json!({"content": "<perf-heavy>Heavy tools are installed.</perf-heavy>"})),
        call("port.claim", json!({"port": 18731})),
        call("config.get", json!({})),
        {"pause": "go-extension"},
        {"once": "extension-ran"},
        call("feature.offer", json!({"features": [{"id": "perf-extra", "name": {"en": "Heavy extra", "zh": "重场景附加"},
            "tools": ["perf_note"]}]})),
        call("session.create", json!({})),
        {"save": "own"},
        call("subscribe", json!({"stream": "events", "session": "{s:own}"})),
        call("session.send", json!({"session": "{s:own}", "text": super::steps::EXTENSION_SAYS})),
        {"turn": "{s:own}"},
        call("session.list", json!({})),
        call("view.page", json!({"session": "{s:own}"})),
        call("session.tools", json!({"session": "{s:own}"})),
        call("events.append", json!({"session": "{s:own}", "kind": "ext.perf-tools.note", "body": {"n": 1}})),
        call("session.set_state", json!({"session": "{s:own}", "state": {"note": "Heavy state.", "data": {"n": 1}}})),
        call("session.state", json!({"session": "{s:own}"})),
        call("widget.set", json!({"widget": "status", "content": [{"type": "text", "text": {"en": "ok", "zh": "好"}, "tone": "accent"}]})),
        call("memory.search", json!({"query": "tea", "persona": "software-engineer"})),
        call("kb.list", json!({})),
        call("model.list", json!({})),
        call("persona.list", json!({})),
        call("skill.list", json!({})),
        call("bus.list", json!({})),
        call("content.list", json!({})),
        call("human.get", json!({})),
        call("port.release", json!({"port": 18731})),
        call("system.append", json!({"content": null})),
    ])
}

/// 平台桥（`perf-bridge`，`platform = "qq"`）的剧本：查两个人是不是主人；三个群各开一个场所会话，每个群里先旁听一句、
/// 判官照这一句查群聊记录（`venue.records`）、用登记的旁路请求问一次模型（`model.call` 的 `side`）、照判下来的回它
/// （`session.respond` 带 `to`），再有人 @ 一句、等回答，记一笔事实、记一次撤回，来一条合并转发，照几块事实开一轮；主人
/// 本人、陌生人各开一个私聊说一句、戳一下；最后列一遍场所会话。说的话带 [`super::steps::BRIDGE_SAYS`]。
pub fn bridge() -> Value {
    let call = |method: &str, params: Value| json!({"call": method, "params": params});
    let says = super::steps::BRIDGE_SAYS;
    let mut steps = vec![
        call("provide", json!({"tools": []})),
        call("venue.sessions", json!({})),
        json!({"pause": "go-bridge"}),
        json!({"once": "bridge-ran"}),
        call("venue.binding", json!({"id": "qq:10001"})),
        call("venue.binding", json!({"id": "qq:20001"})),
    ];
    for g in 1..=3 {
        let venue = format!("qq:group:{g}");
        let name = format!("g{g}");
        let session = format!("{{s:{name}}}");
        let message = |msg: String, more: Value| {
            let mut v = json!({"msg": msg, "name": "小林", "my_name": "Miyu", "place": format!("测试群{g}")});
            if let (Some(v), Some(more)) = (v.as_object_mut(), more.as_object()) {
                v.extend(more.clone());
            }
            v
        };
        steps.extend([
            call("venue.session", json!({"venue": venue, "kind": "group", "persona": "software-engineer", "recent": 50})),
            json!({"save": name}),
            call("subscribe", json!({"stream": "events", "session": session})),
            call("venue.plugins", json!({"venue": venue, "on": []})),
            call("session.send", json!({"session": session, "text": format!("{says} 有人在吗"), "as": {"external": "qq:20002"},
                "venue": message(format!("a{g}"), json!({"ambient": true}))})),
            call("venue.records", json!({"session": session, "msg": "{seq}", "count": 20, "grow": 10})),
            call("model.call", json!({"purpose": "judge", "side": "judge", "session": session,
                "messages": [{"role": "system", "text": "Decide whether to reply."}, {"role": "user", "text": "小王: 有人在吗"}]})),
            call("session.respond", json!({"session": session, "to": ["{seq}"]})),
            json!({"turn": session}),
            call("session.send", json!({"session": session, "text": format!("{says} 你们看过排函数吗"), "as": {"external": "qq:20001"},
                "venue": message(format!("m{g}"), json!({"mentions_me": true, "mentions": ["qq:99"]}))})),
            json!({"turn": session}),
            call("session.note", json!({"session": session, "facts": [{"kind": "delivery", "text": "The last reply was sent."}]})),
            call("events.append", json!({"session": session, "kind": "venue.recalled", "body": {"msg": format!("a{g}"), "by": "qq:20002"}})),
            call("session.send", json!({"session": session, "text": "[forward]", "as": {"external": "qq:20003", "role": "manager"},
                "venue": message(format!("f{g}"), json!({"ambient": true, "forwards": [{"nodes": [{"name": "甲", "text": "一句"},
                    {"name": "乙", "text": "两句", "media": [{"kind": "image", "id": "img1", "index": 1}]}]}]}))})),
            call("session.respond", json!({"session": session, "facts": [{"kind": "judge", "text": "Someone asked about function ordering."}]})),
            json!({"turn": session}),
        ]);
    }
    for peer in ["10001", "30001"] {
        let name = format!("p{peer}");
        let session = format!("{{s:{name}}}");
        steps.extend([
            call("venue.session", json!({"venue": format!("qq:private:{peer}"), "kind": "private", "peer": format!("qq:{peer}"),
                "persona": "software-engineer", "profile": peer == "10001"})),
            json!({"save": name}),
            call("subscribe", json!({"stream": "events", "session": session})),
            call("session.send", json!({"session": session, "text": format!("{says} 在吗"), "as": {"external": format!("qq:{peer}")},
                "venue": {"msg": format!("p{peer}"), "name": "主人", "my_name": "Miyu"}})),
            json!({"turn": session}),
            call("session.send", json!({"session": session, "text": "", "as": {"external": format!("qq:{peer}")},
                "venue": {"msg": format!("poke{peer}"), "name": "路人", "poke": true}})),
            json!({"turn": session}),
        ]);
    }
    steps.push(call("venue.sessions", json!({})));
    Value::Array(steps)
}
