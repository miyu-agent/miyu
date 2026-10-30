//! 回合的配置（施工 PS-1）：包自己的配置项不照这一轮冻结的那一份，照配置服务这一刻的：回合中途改了的，下一次调用就换。

use serde_json::{Map, Value, json};

use super::*;

/// 只认 `draw` 一个包的来源：它的 `endpoint` 是 `endpoint`。
#[derive(Debug)]
struct Draw {
    endpoint: &'static str,
}

impl ConfigSource for Draw {
    fn with_project(&self, _: &str) -> Resolved {
        Resolved::default()
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }

    fn package_settings(&self, package: &str) -> Option<Map<String, Value>> {
        (package == "draw")
            .then(|| Map::from_iter([("endpoint".to_string(), json!(self.endpoint))]))
    }
}

#[tokio::test]
async fn package_settings_follow_the_newest_config_within_a_turn() {
    let (send, configs) =
        watch::channel(Arc::new(Draw { endpoint: "first" }) as Arc<dyn ConfigSource>);
    let turning = Turning::start(configs, "/".to_string()).await;
    let turn: Arc<dyn miyu_tool::SettingsPort> = Arc::clone(turning.current()) as _;
    assert_eq!(
        turn.package("draw").map(Value::Object),
        Some(json!({"endpoint": "first"}))
    );
    send.send(Arc::new(Draw { endpoint: "second" }))
        .expect("会话还收着");
    assert_eq!(
        turn.package("draw").map(Value::Object),
        Some(json!({"endpoint": "second"})),
        "同一轮里下一次调用就是新的"
    );
    assert_eq!(turn.package("other"), None);
    // 自己造的一轮（一次性调用）没有配置服务可看：照造它的那一份。
    let made: Arc<dyn miyu_tool::SettingsPort> = Arc::new(Turn::new(
        Resolved::default(),
        Arc::new(Draw { endpoint: "made" }),
    ));
    assert_eq!(
        made.package("draw").map(Value::Object),
        Some(json!({"endpoint": "made"}))
    );
}
