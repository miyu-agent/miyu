//! 配置变了推给框的（施工 BP-3）：只推它自己的键，别的包的、像它又不是它的不推。

use serde_json::json;

use super::settings_changed;

#[test]
fn only_its_own_keys_are_pushed() {
    let params =
        json!({"keys": {"relay.token": {}, "relayx.a": {}, "other.b": {}, "relay.port": {}}});
    let pushed = settings_changed("relay", &params).expect("有它的项");
    assert_eq!(pushed["method"], "settings.changed");
    assert_eq!(
        pushed["params"]["keys"],
        json!(["relay.port", "relay.token"])
    );
    assert_eq!(
        settings_changed("relay", &json!({"keys": {"other.b": {}}})),
        None
    );
    assert_eq!(settings_changed("relay", &json!({})), None);
}
