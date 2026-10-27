//! 成员的名单（施工 Y-2 下下上，写配置、密钥照人分开以后放开见 Y-2 下下中中）：名单里的都是分发得到的方法；整台机器的、还没照人分开的不在里面；订阅照流挑。

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
        "config.set",
        "secret.set",
        "secret.list",
        "provider.test",
        "model.list",
        // 往自己那一层装、卸、升级（施工 U23 下上）。
        "package.install",
        "package.remove",
        "package.upgrade",
        "package.sync",
    ] {
        assert!(allowed(method, &json!({})), "{method}");
    }
    for method in [
        "package.enable",
        "package.files",
        "package.clean",
        "extension.enable",
        "provider.detect",
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
    assert!(allowed("subscribe", &json!({"stream": "config"})));
    assert!(allowed("config.get", &json!({})));
    assert!(!allowed("subscribe", &json!({"stream": "packages"})));
    assert!(!allowed("subscribe", &json!({})));
}

/// 管理员模式只给人（施工 Y-12 下）：核心拉起的扩展带了 `scope` 一律拒，不带的照它的能力挡（这里放过）。
#[test]
fn extensions_cannot_enter_admin_mode() {
    use std::sync::Arc;
    let (root, _) = crate::test_support::temp_root("gate-scope");
    let core = crate::Core::new(
        root,
        miyu_store::resources::ResourceRoot::at(crate::test_support::resources()),
        Arc::new(miyu_session::testkit::Script::new([])),
        miyu_tool::Catalog::default(),
        None,
        miyu_kernel::id::AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    );
    let account = miyu_kernel::id::AccountId::parse("kit").expect("合写法");
    let asked = |system| super::Asked {
        method: "session.list",
        params: &serde_json::Value::Null,
        system,
        scoped: false,
    };
    let via = Some(&crate::login::Via::Spawned);
    assert_eq!(
        super::refuse(&core, via, &account, &asked(true)).map(|refusal| refusal.reason),
        Some("not_permitted")
    );
    assert_eq!(super::refuse(&core, via, &account, &asked(false)), None);
}
