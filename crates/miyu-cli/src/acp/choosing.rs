//! 能选的模型跟着核心变（施工 ACP-9，`internal/blueprint/cli/acp.md`「模式和配置项」）：每次开、载入、接上会话重问一次
//! `model.list`；订着配置的推送，供应商的模型列表变了（`models.changed`，后台拉到了新装的包带的那一家的列表）、模型这一块的
//! 配置变了（`config.changed` 里有 `providers.`、`models.`、`pools.` 开头的键）、配置的订阅掉了队（`resync`）也重问。问到的和
//! 原来的不一样：编辑器里开着的会话各报一次整份配置项（`config_option_update`）。
//!
//! 以前一个进程只问一次：编辑器头一次开会话时新装的包带的供应商还没拉到列表（核心在后台拉，这一次先照手头的答），之后
//! 一直列不出它的模型。

use serde_json::{Value, json};

use super::options::{self, Choices};
use super::prompt::answered;
use super::server::Server;

/// 配置里这几块变了，能选的模型可能跟着变。
const MODEL_KEYS: [&str; 3] = ["providers.", "models.", "pools."];

impl Server {
    /// 核心推来的、不属于哪个会话的一条：和能选的模型有关的重问，别的不管。
    pub(crate) async fn config_pushed(&mut self, method: &str, params: &Value) {
        let wanted = match method {
            "models.changed" => true,
            "config.changed" => params["keys"].as_object().is_some_and(|keys| {
                keys.keys()
                    .any(|key| MODEL_KEYS.iter().any(|prefix| key.starts_with(prefix)))
            }),
            // 配置的订阅掉了队，停了：重新订阅，漏掉的照整份重问补上。
            "resync" if params["stream"] == "config" => {
                let _sent = self
                    .rpc
                    .send("subscribe", json!({"stream": "config"}))
                    .await;
                true
            }
            _ => false,
        };
        if wanted {
            self.refresh_choices().await;
        }
    }

    /// 重问一次能选的模型：问不到的照旧用上一份。和原来的不一样的，开着的会话各报一次整份配置项。
    pub(crate) async fn refresh_choices(&mut self) {
        let Ok(listed) = answered(self.rpc.call("model.list", json!({})).await) else {
            return;
        };
        let choices = Choices::of(&listed, self.language);
        if self.choices.as_ref() == Some(&choices) {
            return;
        }
        self.choices = Some(choices);
        let open: Vec<String> = self.sessions.keys().cloned().collect();
        for session in open {
            self.options_changed(&session).await;
        }
    }

    /// 报会话 `id` 这时的整份配置项。
    pub(crate) async fn options_changed(&mut self, id: &str) {
        let Some(session) = self.sessions.get(id) else {
            return;
        };
        let choices = self.choices.clone().unwrap_or_default();
        let configured = options::options(self.language, &choices, &session.current);
        let update = json!({"sessionUpdate": "config_option_update", "configOptions": configured});
        self.editor.update(id, update).await;
    }
}
