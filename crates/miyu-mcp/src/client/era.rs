//! 认时代（施工 X-1，`docs/blueprint/mcp.md`「怎么走」第一条）：先探 `server/discover`，答不上的退回 `initialize` 握手；顺手记下
//! 服务的说明、声明了哪些本事、哪几种「变了」要订（施工 PT-1 下）。

use serde_json::{Value, json};

use super::listen::listens;
use super::{Client, Era};
use crate::error::Failed;
use crate::{LEGACY, MODERN};

/// 版本不对（新时代的 `UnsupportedProtocolVersionError`）。
const UNSUPPORTED_VERSION: i64 = -32022;

/// MCP 规范留给自己用的错误码：落在这里的是新时代的服务。
const MODERN_ERRORS: std::ops::RangeInclusive<i64> = -32099..=-32020;

impl Client {
    /// 探：`server/discover`。新时代的照它列的挑版本；版本不对的照错里列的挑；别的错、没答的退回握手。
    pub(super) async fn discover(&mut self) -> Result<(), Failed> {
        let mut version = MODERN[0].to_string();
        for _ in 0..2 {
            let params = json!({"_meta": self.meta(&version)});
            let probed = self
                .link
                .ask("server/discover", params, Some(self.waits.probe), false)
                .await;
            let offered = match probed {
                Ok(result) => {
                    self.instructions = text(&result, "instructions");
                    self.prompts = offers_prompts(&result);
                    self.resources = offers_resources(&result);
                    self.listens = listens(&result);
                    versions(result.get("supportedVersions"))
                }
                Err(Failed::Rpc { code, data, .. }) if code == UNSUPPORTED_VERSION => {
                    versions(data.as_ref().and_then(|data| data.get("supported")))
                }
                Err(Failed::Rpc { code, .. }) if !MODERN_ERRORS.contains(&code) => {
                    return self.shake().await;
                }
                Err(Failed::TimedOut | Failed::Malformed(_)) => return self.shake().await,
                Err(other) => return Err(other),
            };
            let Some(common) = MODERN
                .iter()
                .find(|ours| offered.iter().any(|theirs| theirs == *ours))
            else {
                return Err(Failed::NoCommonVersion(offered));
            };
            if *common == version {
                self.era = Era::Modern(version);
                return Ok(());
            }
            version = (*common).to_string();
        }
        Err(Failed::Malformed(
            "server/discover kept changing its versions".to_string(),
        ))
    }

    /// 旧时代的握手：报我们认得的最新一版，服务答的那一版我们认得就照它说，再发 `notifications/initialized`。
    pub(super) async fn shake(&mut self) -> Result<(), Failed> {
        let params = json!({
            "protocolVersion": LEGACY[0],
            "capabilities": {},
            "clientInfo": {"name": self.hello.name, "version": self.hello.version},
        });
        let result = self
            .link
            .ask("initialize", params, Some(self.waits.answer), false)
            .await?;
        let Some(version) = text(&result, "protocolVersion") else {
            return Err(Failed::Malformed(
                "initialize without protocolVersion".to_string(),
            ));
        };
        if !LEGACY.contains(&version.as_str()) {
            return Err(Failed::NoCommonVersion(vec![version]));
        }
        self.instructions = text(&result, "instructions");
        self.prompts = offers_prompts(&result);
        self.resources = offers_resources(&result);
        self.link.tell("notifications/initialized", json!({}));
        self.era = Era::Legacy(version);
        Ok(())
    }
}

/// 一格字，没有的、不是字的没有。
fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// 回应的本事里有没有 `prompts`（施工 X-39）：是一张表就算。
fn offers_prompts(result: &Value) -> bool {
    result["capabilities"]["prompts"].is_object()
}

/// 回应的本事里有没有 `resources`（施工 X-41）：是一张表就算。
fn offers_resources(result: &Value) -> bool {
    result["capabilities"]["resources"].is_object()
}

/// 一串版本：不是数组的当空的，不是字的不算。
fn versions(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}
