//! 新建时人自己写编号（施工 ID-1，发版前检查第 28 条）：`preset.set`、`persona.set` 带 `new: true` 和编号的，没有这个编号就照它
//! 新建；已经有了的（出厂的、你那一层的）拒 `preset_exists`、`persona_exists`，什么都不改；`new` 不带编号的 `bad_params`。

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::*;

#[tokio::test]
async fn a_new_preset_or_persona_takes_the_id_written_unless_it_is_taken() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let name = json!([{"key": "preset.name", "value": "Mine"}]);

    let made = client
        .call(
            "p1",
            "preset.set",
            json!({"preset": "my-work", "new": true, "changes": name}),
        )
        .await;
    assert_eq!(made["result"]["preset"], "my-work", "{made}");
    for (id, preset) in [("p2", "my-work"), ("p3", "full")] {
        let taken = client
            .call(
                id,
                "preset.set",
                json!({"preset": preset, "new": true, "changes": name}),
            )
            .await;
        assert_eq!(
            taken["error"]["data"]["reason"], "preset_exists",
            "{preset}：{taken}"
        );
    }
    assert!(
        !home.root.path().join("root/presets/full.toml").exists(),
        "出厂的那份没被改"
    );
    let bare = client
        .call("p4", "preset.set", json!({"new": true, "changes": name}))
        .await;
    assert_eq!(bare["error"]["data"]["reason"], "bad_params", "{bare}");

    let persona = json!([{"key": "persona.name", "value": "Cat"}]);
    let made = client
        .call(
            "q1",
            "persona.set",
            json!({"persona": "cat", "new": true, "changes": persona}),
        )
        .await;
    assert_eq!(made["result"]["persona"], "cat", "{made}");
    let taken = client
        .call(
            "q2",
            "persona.set",
            json!({"persona": "cat", "new": true, "changes": persona}),
        )
        .await;
    assert_eq!(
        taken["error"]["data"]["reason"], "persona_exists",
        "{taken}"
    );
}
