//! `config.schema` 里一项的样子（施工 NS-1 从 `methods.rs` 挪出来：那边到了 500 行）：键、类型、选项、默认值、范围、名字和说明。

use serde_json::{Map, Value, json};

use miyu_config::{Item, Kind};

/// 一项在 `config.schema` 里的样子：选项后面暗字写的照 `notes` 查，用不了的照 `usable` 标。
pub(super) fn schema_item(
    item: &Item,
    said: Option<&miyu_config::ItemWords>,
    notes: &dyn Fn(&str) -> Option<String>,
    usable: &dyn Fn(&str) -> bool,
) -> Value {
    let mut map = Map::new();
    map.insert("key".to_string(), json!(item.key));
    map.insert("type".to_string(), json!(item.kind.as_str()));
    // 选项的列表也列出能选的几个（施工 8-7）；几个字之一或者一个模型的（`model_or`，施工 R-5 补）列出那几个字。
    let options = match item.kind {
        Kind::Option(options) | Kind::List(&Kind::Option(options)) | Kind::ModelOr(options) => {
            Some(options)
        }
        _ => None,
    };
    if let Some(options) = options {
        let named: Vec<Value> = options
            .iter()
            .map(|option| {
                let name = said
                    .and_then(|said| said.options.get(*option))
                    .map_or(*option, String::as_str);
                let mut named = json!({"name": name, "value": option});
                // 选项的说明（施工 PC-1，包清单里写了的）：头暗色写在下面。
                if let Some(description) =
                    said.and_then(|said| said.option_descriptions.get(*option))
                {
                    named["description"] = json!(description);
                }
                // 选中时弹的提示（设计 72，施工 DSR-2）：头在人选中这个选项时弹出来。
                if let Some(notice) = said.and_then(|said| said.option_notices.get(*option)) {
                    named["notice"] = json!(notice);
                }
                if let Some(note) = notes(option) {
                    named["note"] = json!(note);
                }
                if !usable(option) {
                    named["available"] = json!(false);
                }
                named
            })
            .collect();
        map.insert("options".to_string(), json!(named));
    }
    if let Some(default) = &item.default {
        map.insert("default".to_string(), default.json());
    }
    match item.kind {
        Kind::Int { min, max } | Kind::Float { min, max } => {
            map.insert("min".to_string(), json!(min));
            map.insert("max".to_string(), json!(max));
        }
        // 时长的范围写成秒（施工 8-7）。
        Kind::Duration { min, max } => {
            map.insert("min".to_string(), json!(min));
            map.insert("max".to_string(), json!(max));
        }
        Kind::Text { max } | Kind::Lines { max } | Kind::English { max } => {
            map.insert("max".to_string(), json!(max));
        }
        Kind::List(inner) => {
            map.insert("element".to_string(), json!(inner.as_str()));
            // 一组字段（施工 PC-2）：当标题的字段、每个字段一张（格同普通项），字照这一项的字里那个字段的。
            if let Kind::Record(record) = inner {
                if let Some(title) = record.title {
                    map.insert("title".to_string(), json!(title));
                }
                let fields: Vec<Value> = record
                    .fields
                    .iter()
                    .map(|field| {
                        let words = said.and_then(|said| said.fields.get(field.key));
                        schema_item(field, words, &|_| None, &|_| true)
                    })
                    .collect();
                map.insert("fields".to_string(), json!(fields));
            }
        }
        _ => {}
    }
    let layers: Vec<&str> = item.layers.iter().map(|layer| layer.as_str()).collect();
    map.insert("layers".to_string(), json!(layers));
    if let Some(tighten) = item.tighten {
        map.insert("tighten".to_string(), json!(tighten.as_str()));
    }
    if let Some(env) = item.env {
        map.insert("env".to_string(), json!(env));
    }
    map.insert("applies".to_string(), json!(item.applies.as_str()));
    map.insert(
        "name".to_string(),
        json!(said.map_or(item.key, |said| said.name.as_str())),
    );
    map.insert(
        "description".to_string(),
        json!(said.map_or("", |said| said.description.as_str())),
    );
    map.insert("page".to_string(), json!(item.ui.page));
    // 包的配置项归哪个包（施工 F-6 上）：照登记的 `package`，不拿组名当包名（施工 F-14：`net_keys`、`source` 填错过）。
    if let Some(package) = item.ui.package {
        map.insert("package".to_string(), json!(package));
    }
    map.insert("group".to_string(), json!(item.ui.group));
    map.insert("common".to_string(), json!(item.ui.common));
    map.insert("control".to_string(), json!(item.ui.control.as_str()));
    // 滑块一格多少（施工 PC-1）。
    if let miyu_config::Control::Slider { step } = item.ui.control {
        map.insert("step".to_string(), json!(step.get()));
    }
    // 下拉加「其他」手写（施工 P-9 补）：建议值照连接的语言那一份字表。
    if let Some(said) = said.filter(|said| !said.suggestions.is_empty()) {
        map.insert("suggestions".to_string(), json!(said.suggestions));
    }
    if item.ui.hidden {
        map.insert("hidden".to_string(), json!(true));
    }
    Value::Object(map)
}
