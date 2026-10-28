//! 假服务答 `tools/call`（施工 X-1，多轮请求施工 PT-1 中，进度施工 PT-1 下）。

use serde_json::{Value, json};

use super::{Script, Stage, complete, text};
use crate::wire;

impl Stage {
    /// 调工具：照名字答（几件不在工具表里的另有用处，见各自的注释）；交回空的是退出。
    pub(super) fn call(
        &mut self,
        script: &Script,
        id: &Value,
        params: &Value,
    ) -> Option<Vec<Vec<u8>>> {
        let said = params["arguments"]["text"].as_str().unwrap_or_default();
        let result = match params["name"].as_str().unwrap_or_default() {
            "echo" => text(said),
            "picture" => json!({"content": [
                {"type": "image", "data": "aGk=", "mimeType": "image/png"},
                {"type": "text", "text": "a picture"},
            ]}),
            "fail" => json!({"content": [{"type": "text", "text": "broke"}], "isError": true}),
            "structured" => json!({"content": [], "structuredContent": {"answer": 42}}),
            "slow" => return Some(Vec::new()),
            "pinged" => {
                self.asking = Some(id.clone());
                return Some(vec![wire::request_raw("p1", "ping")]);
            }
            "ask" => {
                self.asking = Some(id.clone());
                return Some(vec![wire::request_raw("s1", "sampling/createMessage")]);
            }
            "input" => json!({"resultType": "input_required", "inputRequests": {}}),
            // 多轮请求（施工 PT-1 中）：头一回叫重发、带回 `round-2` 才答；`nagging` 一直叫重发。都不在工具表里。
            "busy" if params["requestState"] == "round-2" => text("done after retry"),
            "busy" => json!({"resultType": "input_required", "requestState": "round-2"}),
            "nagging" => json!({"resultType": "input_required", "requestState": "again"}),
            // 同 `busy`，叫重发时带一张空的 `inputRequests`：空的当没要东西。
            "busy-empty" if params["requestState"] == "round-2" => text("done after retry"),
            "busy-empty" => {
                json!({"resultType": "input_required", "inputRequests": {}, "requestState": "round-2"})
            }
            "elicit" => json!({"resultType": "input_required", "requestState": "x",
                "inputRequests": {"name": {"method": "elicitation/create", "params": {"message": "who?"}}}}),
            // 进度（施工 PT-1 下）：带了进度令牌的先推两条进度再答；不在工具表里。
            "steps" => {
                let mut out = Vec::new();
                if let Some(token) = params["_meta"].get("progressToken") {
                    for (done, said) in [(1, "one"), (2, "two")] {
                        out.push(wire::notice(
                            "notifications/progress",
                            json!({"progressToken": token, "progress": done, "total": 3, "message": said}),
                        ));
                    }
                }
                out.push(complete(script, id, text("stepped")));
                return Some(out);
            }
            "die" => return None,
            _ => return Some(vec![wire::answer(id, Err((-32602, "Unknown tool")))]),
        };
        Some(vec![complete(script, id, result)])
    }
}
