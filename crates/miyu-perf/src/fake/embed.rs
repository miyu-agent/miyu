//! 假模型回语义模型的请求（施工 LW-30）：重场景里配了远程的语义模型（`models.embedding`），记忆、知识库照它算向量。请求体是
//! openai 的写法（`model`、`input`，没有 `messages`），回每一段一个 64 维的向量，照字的哈希算，同样的字回同样的向量。

use serde_json::{Value, json};

/// 向量几维。
const DIMENSIONS: usize = 64;

/// 请求体 `body` 是语义模型的请求的，交回回应的 JSON；别的是 `None`。
pub fn answer(body: &[u8]) -> Option<String> {
    let request: Value = serde_json::from_slice(body).ok()?;
    if request.get("messages").is_some() {
        return None;
    }
    let texts: Vec<String> = match request.get("input")? {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().unwrap_or_default().to_string())
            .collect(),
        _ => return None,
    };
    let data: Vec<Value> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| json!({"object": "embedding", "index": index, "embedding": vector(text)}))
        .collect();
    Some(
        json!({"object": "list", "model": request["model"], "data": data,
            "usage": {"prompt_tokens": 8, "total_tokens": 8}})
        .to_string(),
    )
}

/// 一段字的向量：每一维是字的 FNV-1a 哈希接上维数以后的一个数，落在 -1 到 1 之间。
fn vector(text: &str) -> Vec<f64> {
    (0..DIMENSIONS)
        .map(|dimension| {
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            for byte in text.bytes().chain(dimension.to_le_bytes()) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
            f64::from((hash % 2001) as u32) / 1000.0 - 1.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_requests_get_one_vector_per_text_and_chats_do_not() {
        let reply: Value = serde_json::from_str(
            &answer(br#"{"model":"m","input":["a","b"]}"#).expect("是语义模型的请求"),
        )
        .expect("是 JSON");
        assert_eq!(reply["data"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            reply["data"][1]["embedding"].as_array().map(Vec::len),
            Some(DIMENSIONS)
        );
        assert_eq!(vector("a"), vector("a"));
        assert_ne!(vector("a"), vector("b"));
        assert!(answer(br#"{"model":"m","messages":[],"input":"x"}"#).is_none());
        assert!(answer(b"not json").is_none());
    }
}
