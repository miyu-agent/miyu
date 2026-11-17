//! 配置在协议上的几种写法（`docs/blueprint/config.md`「最终值和来源」「报错」）：来源、一条问题、现在照什么用着。
//! 对象的格照名字的字母先后排：`serde_json` 的 `Map` 本来就这样排。

use serde_json::{Map, Value, json};

use miyu_config::Layer;
use miyu_config::merge::Origin;
use miyu_config::problem::{Problem, Told, Using};

/// 配置项 `key` 的来源：出厂的、借得到供应商的 key 的写「已继承」（施工 NS-1，[`super::inherited`]），别的同 [`origin`]。
/// `values` 是这时的最终值。
pub(super) fn origin_of(
    key: &str,
    origin: &Origin,
    shown: &dyn Fn(Layer) -> Option<String>,
    values: &miyu_config::Values,
) -> Value {
    super::inherited::origin(key, Some(origin), values)
        .unwrap_or_else(|| self::origin(origin, shown))
}

/// 来源：`{"file","layer","line"}`、`{"layer":"env","name"}`、`{"layer":"default"}`、`{"layer":"package","package"}`（施工 PV-4：
/// 软件包带的）。文件照 `shown` 找这一层的写法。
pub(super) fn origin(origin: &Origin, shown: &dyn Fn(Layer) -> Option<String>) -> Value {
    match origin {
        Origin::Default => json!({"layer": "default"}),
        Origin::Env(name) => json!({"layer": "env", "name": name}),
        Origin::Package(package) => json!({"layer": "package", "package": package}),
        Origin::File { layer, line } => {
            let mut map = Map::new();
            if let Some(file) = shown(*layer) {
                map.insert("file".to_string(), json!(file));
            }
            map.insert("layer".to_string(), json!(layer.as_str()));
            map.insert("line".to_string(), json!(line));
            Value::Object(map)
        }
    }
}

/// 一条问题：原因码、级别、整句的话；有的才写：文件、行、列、键、期望、收到、最近的键名、现在照什么用着。
pub(super) fn problem(
    problem: &Problem,
    file: Option<&str>,
    told: &Told,
    using: Option<&Using>,
) -> Value {
    let mut map = Map::new();
    map.insert("code".to_string(), json!(problem.code.as_str()));
    map.insert("level".to_string(), json!(problem.severity().as_str()));
    map.insert("message".to_string(), json!(told.message));
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            map.insert(key.to_string(), value);
        }
    };
    put("file", file.map(|file| json!(file)));
    put("line", problem.at.map(|at| json!(at.line)));
    put("column", problem.at.map(|at| json!(at.column)));
    put("key", problem.key.as_ref().map(|key| json!(key)));
    put(
        "expected",
        told.expected.as_ref().map(|expected| json!(expected)),
    );
    // 照说成话的那一份：密钥项是它是什么样的（施工 SEC-1），不是 `problem.got`。
    put("got", told.got.as_ref().map(|got| json!(got)));
    put("suggest", problem.suggest.map(|suggest| json!(suggest)));
    put("using", using.map(using_json));
    Value::Object(map)
}

/// 现在照什么用着：`{"from","value"}`、`{"from":"last_good"}`、`{"from":"nothing"}`。
fn using_json(using: &Using) -> Value {
    match using {
        Using::Value(value, origin) => {
            json!({"from": origin.layer_name(), "value": value.json()})
        }
        Using::LastGood => json!({"from": "last_good"}),
        Using::Nothing => json!({"from": "nothing"}),
    }
}
