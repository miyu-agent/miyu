//! 头读回条目时，内核里「先整块读原文、再照 `kind` 分派」的几样（`By`、`RawJson`）经通用的 JSON 转一道（施工 V-D）：条目照
//! `kind` 分派、`flatten`，serde 读的时候先把内容攒成一份中间的样子，那几样只认得从 JSON 原文读，读中间的样子会报「newtype
//! struct」。读成 `serde_json::Value` 再交给它们（`Value` 认得它们要的写法），一样的结果。只在读的时候用，写出去的一个字节不变。

use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// 一格：读成 `Value` 再交给它。
pub(crate) fn one<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(d)?;
    serde_json::from_value(value).map_err(D::Error::custom)
}

/// 可以没有的一格：有的读成 `Value` 再交给它。
pub(crate) fn some<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    match Option::<Value>::deserialize(d)? {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(D::Error::custom),
        None => Ok(None),
    }
}
