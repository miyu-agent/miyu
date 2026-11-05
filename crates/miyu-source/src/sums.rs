//! 发版的校验和清单 `SHA256SUMS`（施工 RL-2，设计 `59-发版.md` 第四节）：每个压缩包一行 `<64 位小写十六进制>  <文件名>`，
//! 同 `sha256sum` 写的、`sha256sum -c` 认的样子。签之前照这里查一遍，写坏的不签（签法同索引，[`crate::sign`]，另一把钥匙）。

/// 查 `text` 是不是一份能签的清单，交回有几行。
///
/// # Errors
///
/// 是空的；不是 UTF-8 的整份由调用的一方先拒；某一行不是「64 位小写十六进制、两个空格、文件名」；文件名带路径分隔符、是 `.`、`..`；
/// 同一个文件名写了两行；最后没有换行。错误里写第几行。
pub fn check(text: &str) -> Result<usize, String> {
    if text.is_empty() {
        return Err("SHA256SUMS is empty".to_string());
    }
    if !text.ends_with('\n') {
        return Err("SHA256SUMS does not end with a newline".to_string());
    }
    let mut names = std::collections::BTreeSet::new();
    let mut count = 0;
    for (number, line) in text.lines().enumerate() {
        let number = number + 1;
        let (hash, name) = line
            .split_once("  ")
            .ok_or_else(|| format!("line {number}: not `<sha256>  <file>`"))?;
        let hex = hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex {
            return Err(format!(
                "line {number}: the hash is not 64 lowercase hex digits"
            ));
        }
        let plain = !name.is_empty()
            && name != "."
            && name != ".."
            && !name.contains(['/', '\\'])
            && !name.starts_with(' ')
            && !name.chars().any(char::is_control);
        if !plain {
            return Err(format!("line {number}: `{name}` is not a plain file name"));
        }
        if !names.insert(name) {
            return Err(format!("line {number}: `{name}` is listed twice"));
        }
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests;
