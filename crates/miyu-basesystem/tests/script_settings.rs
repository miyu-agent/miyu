//! 脚本包的配置项（施工 PS-1）：包自己 `scripts/` 下的脚本跑的时候拿到这个包配置项的最终值（`MIYU_SETTINGS_JSON`，键去掉包的
//! 配置前缀）；别的包的脚本拿的是它自己那份，家目录里人自己放的脚本、她跑的 `shell` 都没有。每次调用照端口现取：改了的下一次就换。

use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};

use miyu_config::secret::{Reference, Secret};
use miyu_tool::{Call, Progress, Seen, SettingsPort, Stop};

use crate::script::text;
use crate::support::Site;

/// 印出 `MIYU_SETTINGS_JSON`，没有的印 `unset`。
const SHOW: &[u8] = b"#!/usr/bin/env python3\n# Description: Show settings.\nimport os\nprint(os.environ.get('MIYU_SETTINGS_JSON', 'unset'))\n";

/// 假的配置：`draw` 包的照 `draw` 那一格（测试里改它），`other` 包的另一份，家目录那一份也有，别的包没有配置项。
struct Settings {
    draw: Mutex<Map<String, Value>>,
}

impl SettingsPort for Settings {
    fn values(&self) -> miyu_config::Values {
        miyu_config::Values::default()
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }

    fn package(&self, package: &str) -> Option<Map<String, Value>> {
        match package {
            "draw" => Some(self.draw.lock().expect("没毒").clone()),
            "other" => Some(Map::from_iter([("x".to_string(), json!(1))])),
            // 端口就算有，家目录里人自己放的脚本也不给（脚本那一头自己挡）。
            miyu_store::scripts::HOME_PACKAGE => {
                Some(Map::from_iter([("y".to_string(), json!(2))]))
            }
            _ => None,
        }
    }
}

/// 在 `work/` 里跑场地里的 `file`，来处是 `source`，带着配置端口 `settings`；交回印出来的那一行。
async fn shown(site: &Site, file: &str, source: &str, settings: &Arc<Settings>) -> String {
    let tool = site.script_tool_from(file, source);
    let call = Call {
        settings: Some(Arc::clone(settings) as Arc<dyn SettingsPort>),
        ..site.call_for("work", json!({}), Seen::new(), Stop::default())
    };
    let done = tool.run(call, Progress::new(|_| {})).await;
    assert!(!done.error, "{}", text(&done));
    text(&done).trim_end().to_string()
}

#[tokio::test]
async fn a_package_script_gets_its_own_settings_and_others_do_not() {
    let site = Site::new();
    site.file("scripts/show.py", SHOW);
    let settings = Arc::new(Settings {
        draw: Mutex::new(Map::from_iter([
            ("endpoint".to_string(), json!("https://example.com/v1")),
            ("key".to_string(), json!("sk-real")),
            ("style.size".to_string(), json!(512)),
        ])),
    });
    let read = |printed: String| -> Value {
        serde_json::from_str(&printed).unwrap_or_else(|_| panic!("不是 JSON：{printed}"))
    };
    assert_eq!(
        read(shown(&site, "scripts/show.py", "draw", &settings).await),
        json!({"endpoint": "https://example.com/v1", "key": "sk-real", "style.size": 512}),
        "包自己的脚本拿到它的那一份"
    );
    assert_eq!(
        read(shown(&site, "scripts/show.py", "other", &settings).await),
        json!({"x": 1}),
        "别的包的脚本拿它自己那一份"
    );
    assert_eq!(
        shown(
            &site,
            "scripts/show.py",
            miyu_store::scripts::HOME_PACKAGE,
            &settings
        )
        .await,
        "unset",
        "家目录里人自己放的没有"
    );
    assert_eq!(
        shown(&site, "scripts/show.py", "plain", &settings).await,
        "unset",
        "没有配置项的包没有"
    );
    settings
        .draw
        .lock()
        .expect("没毒")
        .insert("endpoint".to_string(), json!("https://other.example.com"));
    assert_eq!(
        read(shown(&site, "scripts/show.py", "draw", &settings).await)["endpoint"],
        "https://other.example.com",
        "改了的下一次调用就换"
    );
}

/// 她跑的命令不带（施工 PS-1）：只有包自己的脚本拿得到。
#[cfg(unix)]
#[tokio::test]
async fn shell_commands_do_not_get_package_settings() {
    let site = Site::new();
    let settings = Arc::new(Settings {
        draw: Mutex::new(Map::from_iter([("key".to_string(), json!("sk-real"))])),
    });
    let call = Call {
        settings: Some(Arc::clone(&settings) as Arc<dyn SettingsPort>),
        ..site.call_for(
            "work",
            json!({"command": "echo ${MIYU_SETTINGS_JSON:-unset}", "description": "show"}),
            Seen::new(),
            Stop::default(),
        )
    };
    let done = crate::support::tool("shell")
        .run(call, Progress::new(|_| {}))
        .await;
    assert!(text(&done).contains("unset"), "{}", text(&done));
}
