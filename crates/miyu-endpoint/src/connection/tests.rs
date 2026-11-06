//! 扩展能调什么的表（施工 9-4 上、P-3 中、T-9 起的只给人的名单，施工 X-30 改成照能力挡）：真起一个扩展进程的在
//! `tests/extensions.rs`、`tests/extension_rights.rs`，这里守表。

use serde_json::{Value, json};

use miyu_config::package::Capability;

use super::rights::{Right, right};

/// 不带参数时扩展调 `method` 要什么。
fn bare(method: &str) -> Right {
    right(method, &Value::Null)
}

#[test]
fn people_only_stays_people_only() {
    for method in [
        "extension.enable",
        "extension.restart",
        "preset.set",
        "preset.delete",
        "persona.set",
        "persona.delete",
        "package.install",
        "package.remove",
        "package.enable",
        "package.disable",
        "package.restart",
        "package.sync",
        "package.search",
        "package.clean",
        "package.upgrades",
        "package.upgrade",
        "package.file",
        "mcp.list",
        "mcp.restart",
        "mcp.set",
        "mcp.remove",
        "skill.remove",
        "tool.set_loading",
        "feature.set",
        "config.trust",
        "secret.set",
        "secret.delete",
        "project.list",
        "project.create",
        "project.update",
        "project.delete",
        // 头自己的设置（施工 HP-1）。
        "prefs.get",
        "prefs.set",
        // 施工 X-30：替人回答、改权限级别和工作目录、终端会话、访问密码和登录（删会话施工 Y-2 上起要 `sessions.drive`）。
        "session.answer",
        "session.set_permission_level",
        "session.set_workspace",
        "session.terminal",
        "session.set_terminal",
        "account.setup_code",
        "core.stop",
        "login.ticket",
        // 表里没有的。
        "no.such_method",
        "tool.call",
    ] {
        assert_eq!(bare(method), Right::People, "{method}");
    }
}

#[test]
fn capabilities_gate_what_they_name() {
    for (method, capability) in [
        ("session.create", Capability::SessionsDrive),
        ("session.delete", Capability::SessionsDrive),
        ("session.send", Capability::SessionsDrive),
        // 不带参数的 `session.respond` 是只带事实开一轮（施工 R3S-3）。
        ("session.respond", Capability::ActForExternal),
        ("command.run", Capability::SessionsDrive),
        ("session.list", Capability::EventsRead),
        ("view.page", Capability::EventsRead),
        ("session.prompts", Capability::EventsRead),
        ("events.append", Capability::EventsWrite),
        ("session.note", Capability::ActForExternal),
        ("venue.plugins", Capability::ActForExternal),
        ("venue.session", Capability::ActForExternal),
        ("memory.remember", Capability::MemoryWrite),
        ("kb.add", Capability::MemoryWrite),
        ("fs.read", Capability::FsRead),
        ("link.preview", Capability::Network),
        ("widget.set", Capability::Widgets),
        ("system.append", Capability::ContextSystem),
        ("package.call", Capability::PackagesCall),
    ] {
        assert_eq!(bare(method), Right::Needs(capability), "{method}");
    }
}

#[test]
fn some_hang_on_the_params() {
    let acting = json!({"as": {"platform": "qq", "id": "1"}});
    assert_eq!(
        right("session.send", &acting),
        Right::Needs(Capability::ActForExternal)
    );
    assert_eq!(
        right("command.run", &acting),
        Right::Needs(Capability::ActForExternal)
    );
    assert_eq!(
        right("session.respond", &json!({"to": [3]})),
        Right::Needs(Capability::SessionsDrive),
        "照记下的几条开一轮的照旧"
    );
    assert_eq!(
        right("session.respond", &json!({"to": []})),
        Right::Needs(Capability::ActForExternal),
        "空的 to 同不写（施工 R3S-3）"
    );
    assert_eq!(
        right("session.send", &json!({"as": null})),
        Right::Needs(Capability::SessionsDrive),
        "写成 null 的当没写"
    );
    assert_eq!(
        right("events.append", &json!({"kind": "venue.recalled"})),
        Right::Needs(Capability::ActForExternal)
    );
    assert_eq!(
        right("events.append", &json!({"kind": "ext.echo.a"})),
        Right::Needs(Capability::EventsWrite)
    );
    assert_eq!(
        right("package.info", &json!({"path": "/tmp/kit"})),
        Right::People,
        "看本机的包文件夹同装，只给人（施工 IN-4）"
    );
    assert_eq!(
        right("package.info", &json!({"package": "kit"})),
        Right::Anyone
    );
    assert_eq!(
        right("provide", &json!({"tools": []})),
        Right::Anyone,
        "空的照收"
    );
    assert_eq!(
        right("provide", &json!({"tools": [{"name": "a"}]})),
        Right::Needs(Capability::Tools)
    );
    // 替本机页面换票（施工 LP-1）：只认 `via` 是 `page` 的，别的、不写的、不是字的照旧只给人。
    assert_eq!(
        right("login.ticket", &json!({"via": "page"})),
        Right::Needs(Capability::LoginPage)
    );
    for params in [
        json!({"via": "open"}),
        json!({"via": "Page"}),
        json!({"via": ["page"]}),
        json!({"via": null}),
        json!({}),
    ] {
        assert_eq!(right("login.ticket", &params), Right::People, "{params}");
    }
    for (stream, wanted) in [
        ("events", Right::Needs(Capability::EventsRead)),
        ("view", Right::Needs(Capability::EventsRead)),
        ("sessions", Right::Needs(Capability::EventsRead)),
        ("projects", Right::People),
        ("prefs", Right::People),
        ("config", Right::Anyone),
        ("packages", Right::Anyone),
        ("memory", Right::Anyone),
    ] {
        assert_eq!(
            right("subscribe", &json!({"stream": stream})),
            wanted,
            "{stream}"
        );
    }
}

#[test]
fn reading_stays_open() {
    for method in [
        "hello",
        "human.get",
        "preset.get",
        "persona.get",
        "persona.read",
        "package.list",
        // 施工 T-9 补：扩展只能改自己包编号开头的几项，在 `config::set::set_from_package` 里挡。
        "config.set",
        "config.get",
        "config.schema",
        "config.check",
        "secret.list",
        "skill.list",
        "script.list",
        "memory.search",
        "kb.search",
        "blob.put",
        "model.call",
        "login.redeem",
    ] {
        assert_eq!(bare(method), Right::Anyone, "{method}");
    }
}
