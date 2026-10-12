//! 部首的码位换回平常的字（施工 K-5）：中文 PDF 常把字映射成康熙部首、部首补充那两段的码位（Chrome 打出来的 PDF 里「子」是
//! U+2F26 `⼦`、「页」是 U+2EDA `⻚`），看着一样，搜不到。照 Unicode 的 `EquivalentUnifiedIdeograph.txt`（16.0.0，原样带在
//! `crates/miyu-extract/data/`，Unicode 许可证）换，别的字不动。不用 NFKC：它换不了部首补充那一段，还会把全角标点换成半角。

use std::collections::HashMap;
use std::sync::OnceLock;

/// Unicode 的对照表原文：一行 `<码位或码位..码位> ; <平常的字> # 说明`，`#` 起是注释。
const TABLE: &str = include_str!("../data/EquivalentUnifiedIdeograph.txt");

/// `text` 里部首的码位换成平常的字。
pub(crate) fn plain(text: &str) -> String {
    let table = table();
    text.chars()
        .map(|c| table.get(&c).copied().unwrap_or(c))
        .collect()
}

/// 对照表，头一次用时读。
fn table() -> &'static HashMap<char, char> {
    static TABLE_READ: OnceLock<HashMap<char, char>> = OnceLock::new();
    TABLE_READ.get_or_init(|| read(TABLE))
}

/// 读对照表的原文；读不懂的行不要（原文是带进来的，测试照它全读得懂）。
fn read(text: &str) -> HashMap<char, char> {
    let mut table = HashMap::new();
    for line in text.lines() {
        let data = line.split('#').next().unwrap_or_default();
        let Some((from, to)) = data.split_once(';') else {
            continue;
        };
        let Some(to) = code(to) else {
            continue;
        };
        let (first, last) = match from.trim().split_once("..") {
            Some((first, last)) => (code(first), code(last)),
            None => (code(from), code(from)),
        };
        let (Some(first), Some(last)) = (first, last) else {
            continue;
        };
        for c in first..=last {
            table.insert(c, to);
        }
    }
    table
}

/// 十六进制的一个码位。
fn code(hex: &str) -> Option<char> {
    u32::from_str_radix(hex.trim(), 16)
        .ok()
        .and_then(char::from_u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radicals_become_the_plain_characters_and_nothing_else_changes() {
        assert_eq!(plain("团⼦⼀只，第三⻚。ＡＢ"), "团子一只，第三页。ＡＢ");
        let table = read(TABLE);
        let lines = TABLE
            .lines()
            .filter(|line| !line.starts_with('#') && line.contains(';'))
            .count();
        assert!(table.len() >= lines, "原文每一行都读得懂");
        assert_eq!(table.get(&'\u{2F00}'), Some(&'一'));
    }
}
