//! 别处来的话的来处（`user` 条目的 `from`）：写法同 `by`；场所里的人说的再多一格 `name`（施工 VW-2，`docs/blueprint/view.md`
//! 「条目」的 `user`）：那条消息的 `venue.name`，只当显示名，不可信，头照常转义。名字会变，每条各写各的。
//!
//! `By` 是内核里「先整块读原文、再照 `kind` 分派」的那种（`relay.rs`），不能和别的格 `flatten` 在一起：写的时候先写成 JSON
//! 再接上 `name`，读的时候先拿掉 `name` 再交给它。别的 harness 的 `by` 自己就有 `name`（它报的名字）：写的时候不盖，读的时候
//! 拿掉了读不成就照原样读。

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use miyu_kernel::origin::By;

/// 一条话的来处。
#[derive(Debug, Clone, PartialEq)]
pub struct Sender {
    /// 谁说的，同事件的 `by`。
    pub by: By,
    /// 场所里的人那条消息里写着的名字（群名片、昵称）；没写的、不是场所消息的没有。
    pub name: Option<String>,
}

impl Serialize for Sender {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serde_json::to_value(&self.by).map_err(serde::ser::Error::custom)?;
        // 别的 harness 的 `by` 自己就有 `name`：那一格不动。
        if let (Some(name), Value::Object(fields)) = (&self.name, &mut value) {
            fields
                .entry("name")
                .or_insert_with(|| Value::String(name.clone()));
        }
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Sender {
    /// 先拿掉 `name` 读 `by`；读不成的是 `by` 自己就有 `name` 这一格（别的 harness 的名字），照原样读、没有显示名。
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Sender, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let mut stripped = value.clone();
        let name = match &mut stripped {
            Value::Object(fields) => fields.remove("name"),
            _ => None,
        };
        if let Ok(by) = serde_json::from_value(stripped) {
            let name = match name {
                Some(Value::String(name)) => Some(name),
                Some(_) => return Err(serde::de::Error::custom("name is not a string")),
                None => None,
            };
            return Ok(Sender { by, name });
        }
        let by = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Sender { by, name: None })
    }
}
