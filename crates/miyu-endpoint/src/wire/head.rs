//! 太长的那一行开头里的 `id`（施工 YC-5，压测 F5、O11）：一行超过 [`super::LINE_LIMIT`] 不解析，可回的拒绝要带上能对上号的
//! `id`，不然头、第三方客户端对不上是哪一条，一直等。只看开头 [`HEAD`] 字节，照括号的层数认最外层那一格 `id`（`params` 里
//! 的 `id` 不算），值是字符串、数字的才算，截在半路的、别的写法的当认不出。

use serde_json::Value;

/// 看开头多少字节：请求照惯例把 `jsonrpc`、`id`、`method` 写在前面。
pub(super) const HEAD: usize = 4096;

/// `head` 里最外层那一格 `id` 的值；认不出的是空的。
pub(super) fn id_in(head: &[u8]) -> Option<Value> {
    let mut depth = 0usize;
    let mut at = 0;
    while at < head.len() {
        match head[at] {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.checked_sub(1)?,
            b'"' => {
                let end = string_end(head, at)?;
                if depth == 1 && &head[at + 1..end] == b"id" {
                    let mut next = end + 1;
                    while head.get(next).is_some_and(u8::is_ascii_whitespace) {
                        next += 1;
                    }
                    if head.get(next) == Some(&b':') {
                        return value(&head[next + 1..]);
                    }
                }
                at = end;
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// 从 `start` 那个引号起的字符串，收尾的引号在哪；没收尾的是空的。
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut at = start + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'"' => return Some(at),
            _ => at += 1,
        }
    }
    None
}

/// `rest` 开头的一个 JSON 值：字符串、数字的交回，别的、读不完的是空的。
fn value(rest: &[u8]) -> Option<Value> {
    let mut values = serde_json::Deserializer::from_slice(rest).into_iter::<Value>();
    match values.next()? {
        Ok(found @ (Value::String(_) | Value::Number(_))) => Some(found),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::id_in;

    #[test]
    fn the_top_level_id_is_read_from_the_head() {
        let line = br#"{"jsonrpc":"2.0","id":"c-7","method":"session.send","params":{"text":"aaaa"#;
        assert_eq!(id_in(line), Some(json!("c-7")));
        assert_eq!(id_in(br#"{ "id" : 42, "method": "x""#), Some(json!(42)));
    }

    #[test]
    fn an_id_inside_params_is_not_taken() {
        let line = br#"{"jsonrpc":"2.0","params":{"id":"inner","text":"aaaa"#;
        assert_eq!(id_in(line), None);
        let later = br#"{"params":{"id":"inner"},"id":"outer","method":"x""#;
        assert_eq!(id_in(later), Some(json!("outer")));
    }

    #[test]
    fn a_cut_or_odd_id_is_not_taken() {
        assert_eq!(id_in(br#"{"jsonrpc":"2.0","id":"c-"#), None);
        assert_eq!(id_in(br#"{"id":{"x":1},"method":"x""#), None);
        assert_eq!(id_in(br#"{"method":"id","x":1"#), None);
        assert_eq!(
            id_in(br#"{"text":"say \"id\": 1","id":"c-1""#),
            Some(json!("c-1"))
        );
        assert_eq!(id_in(b"not json at all"), None);
    }
}
