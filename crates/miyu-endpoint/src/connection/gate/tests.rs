//! 成员的名单（施工 Y-2 下下上）：名单里的都是分发得到的方法；整台机器的、还没照人分开的不在里面；订阅照流挑。

use serde_json::json;

use super::members::allowed;

#[test]
fn members_get_their_own_things_and_nothing_machine_wide() {
    for method in [
        "session.send",
        "memory.search",
        "kb.delete",
        "usage.query",
        "preset.set",
    ] {
        assert!(allowed(method, &json!({})), "{method}");
    }
    for method in [
        "config.set",
        "config.get",
        "package.install",
        "extension.enable",
        "secret.list",
        "provider.test",
        "fs.read",
        "check",
        "mcp.set",
        "model.call",
        "link.preview",
        "account.invite",
    ] {
        assert!(!allowed(method, &json!({})), "{method}");
    }
    assert!(allowed("subscribe", &json!({"stream": "view"})));
    assert!(allowed("subscribe", &json!({"stream": "memory"})));
    assert!(!allowed("subscribe", &json!({"stream": "config"})));
    assert!(!allowed("subscribe", &json!({"stream": "packages"})));
    assert!(!allowed("subscribe", &json!({})));
}
