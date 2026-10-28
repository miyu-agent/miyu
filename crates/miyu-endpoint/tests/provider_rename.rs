//! 供应商改名（施工 8-34，`docs/blueprint/models.md`「供应商改名」）：表连同子表、密钥的引用换名，指着旧编号的引用、
//! 池的成员都换，别处的字照原样；会话照的改名记下来；新编号已有的、旧编号没有的、写法不对的拒，什么都不写。

use serde_json::{Value, json};

use crate::support::providers::{core, data, profiles};
use crate::support::*;

const SYSTEM: &str = "config.toml";

fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).unwrap_or_default()
}

async fn rename(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "provider.rename", params).await
}

#[tokio::test]
async fn the_table_and_every_reference_follow_the_new_name() {
    let home = Home::new();
    home.write(
        SYSTEM,
        "# 系统的\n[providers.old]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\nkey = { secret = \"k\" }\n\n[providers.old.models.m]\nwindow = 8000\n\n[providers.other]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\n[models]\nchat = \"old/m\"\nvision = \"old/v\"  # 看图的\n\n[pools.p]\nmodels = [\"old/m\", \"other/x\"]\n",
    );
    let models = data(profiles(json!({})));
    let mut client = Client::connect(core(&home, &[], std::sync::Arc::clone(&models)));
    client.hello().await;

    let reply = rename(&mut client, "r1", json!({"from": "old", "to": "new"})).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let system = read(&home, SYSTEM);
    assert!(
        system.starts_with("# 系统的\n[providers.new]\n"),
        "{system}"
    );
    for kept in [
        "[providers.new.models.m]\nwindow = 8000\n",
        "key = { secret = \"k\" }",
        "chat = \"new/m\"",
        "models = [\"new/m\", \"other/x\"]",
        "[providers.other]",
        "vision = \"new/v\"  # 看图的\n",
    ] {
        assert!(system.contains(kept), "{kept}：{system}");
    }
    assert!(!system.contains("old"), "{system}");
    assert_eq!(
        models.renamed("old").as_deref(),
        Some("new"),
        "会话照的改名记下了"
    );

    // 拒的什么都不写。
    let system = read(&home, SYSTEM);
    for (id, params, expected) in [
        (
            "r2",
            json!({"from": "new", "to": "other"}),
            "provider_exists",
        ),
        ("r3", json!({"from": "old", "to": "x"}), "unknown_provider"),
        ("r4", json!({"from": "new", "to": "Bad"}), "bad_params"),
        ("r5", json!({"from": "new", "to": "new"}), "bad_params"),
        ("r6", json!({"from": "new"}), "bad_params"),
    ] {
        let reply = rename(&mut client, id, params).await;
        assert_eq!(reason(&reply), Some(expected), "{reply}");
    }
    assert_eq!(read(&home, SYSTEM), system);
}
