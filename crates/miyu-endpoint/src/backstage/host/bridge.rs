//! 宿主页替框调核心（施工 BP-3）：只认 `package-pages.md`「框和网页之间怎么说」那四样，包一律照票认，框说自己是谁不算数。
//! 照网页 `frame.js` 那一套办（深浅由宿主页自己照系统补）：
//!
//! - `context`：`{"package", "language"}`，`language` 是要票的那个连接的语言。
//! - `settings.get`：`config.schema` 里 `package` 是它的项，和 `config.get` 的最终值（没写的照默认，密钥是引用不是值）。
//! - `settings.set`：键要都是 `<编号>.` 开头，不是的整个不办（`forbidden`）；密钥类型给一段字的先 `secret.set` 存成新名字的密钥，
//!   再写引用；照 `config.set` 写进系统配置。
//! - `call`：转 `package.call`。

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use super::Ticket;
use super::link::Link;

/// 答框的一条（已经认成 JSON-RPC 的请求）：交回整条回应。
pub(super) async fn answer(link: &Link, ticket: &Ticket, message: &Value) -> Value {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return reply(id, Err(error(-32600, "Invalid Request")));
    };
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "context" => Ok(json!({"package": ticket.package, "language": ticket.language})),
        "settings.get" => settings(link, &ticket.package).await,
        "settings.set" => write(link, &ticket.package, &params["changes"]).await,
        "call" => call(link, &ticket.package, &params).await,
        _ => Err(error(-32601, "Method not found")),
    };
    reply(id, result)
}

fn reply(id: Value, result: Result<Value, Value>) -> Value {
    match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    }
}

fn error(code: i64, message: &str) -> Value {
    json!({"code": code, "message": message})
}

/// 配置清单里 `package` 是它的那几项。
async fn items(link: &Link, package: &str) -> Result<Vec<Value>, Value> {
    let schema = link.request("config.schema", json!({})).await?;
    Ok(schema["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["package"] == package)
        .cloned()
        .collect())
}

async fn settings(link: &Link, package: &str) -> Result<Value, Value> {
    let items = items(link, package).await?;
    let keys: Vec<&Value> = items.iter().map(|item| &item["key"]).collect();
    let got = match keys.is_empty() {
        true => json!({"items": {}}),
        false => link.request("config.get", json!({"keys": keys})).await?,
    };
    let values: Map<String, Value> = items
        .iter()
        .filter_map(|item| {
            let key = item["key"].as_str()?;
            let value = got["items"][key]
                .get("value")
                .filter(|value| !value.is_null())
                .or_else(|| item.get("default"))
                .cloned()
                .unwrap_or(Value::Null);
            Some((key.to_string(), value))
        })
        .collect();
    Ok(json!({"items": items, "values": values}))
}

async fn write(link: &Link, package: &str, changes: &Value) -> Result<Value, Value> {
    let changes = changes
        .as_array()
        .filter(|changes| !changes.is_empty() && changes.iter().all(|one| one["key"].is_string()))
        .ok_or_else(|| error(-32602, "Invalid params"))?;
    let prefix = format!("{package}.");
    if changes.iter().any(|one| {
        !one["key"]
            .as_str()
            .is_some_and(|key| key.starts_with(&prefix))
    }) {
        return Err(
            json!({"code": -32010, "message": "forbidden", "data": {"reason": "forbidden"}}),
        );
    }
    let items = items(link, package).await?;
    let secret = |key: &str| {
        items
            .iter()
            .any(|item| item["key"] == key && item["type"] == "secret")
    };
    let mut out = Vec::with_capacity(changes.len());
    for change in changes {
        let key = change["key"].as_str().unwrap_or_default();
        match change["value"].as_str().filter(|_| secret(key)) {
            Some(value) => {
                let name = secret_name(key);
                link.request("secret.set", json!({"name": name, "value": value}))
                    .await?;
                out.push(json!({"key": key, "value": {"secret": name}}));
            }
            None => out.push(change.clone()),
        }
    }
    link.request("config.set", json!({"layer": "system", "changes": out}))
        .await
}

async fn call(link: &Link, package: &str, params: &Value) -> Result<Value, Value> {
    let mut asked = json!({"package": package, "method": params["method"]});
    if let Some(given) = params.get("params") {
        asked["params"] = given.clone();
    }
    link.request("package.call", asked).await
}

/// 新密钥的名字（同网页的 `secretName`）：照键起，小写、别的字换成 `-`，带上这一刻（三十六进制的毫秒），最多 64 个字。
fn secret_name(key: &str) -> String {
    let mut name = String::new();
    let mut gap = false;
    for c in key.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            name.push(c);
            gap = false;
        } else if !gap {
            name.push('-');
            gap = true;
        }
    }
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    name.push('-');
    name.push_str(&base36(millis));
    name.chars().take(64).collect()
}

fn base36(mut n: u128) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[usize::try_from(n % 36).unwrap_or(0)]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests;
