//! 驱动家族的写法（施工 MD-2 从 `id.rs` 挪出来：那边到了行数上限）。

/// 驱动家族（施工 MD-2）：内置的照 [`super::check_name`]；带 `/` 的是扩展提供的，`<包编号>/<驱动名>` 两段，各是小写字母开头、只用小写字母、
/// 数字、`-`、`_`，各最长 64 个字符（包编号、驱动名的规则，`miyu-config` 的清单查过）。以前的数据都没有 `/`，照旧读得进。
pub(super) fn check_family(text: &str) -> Result<(), &'static str> {
    let Some((package, driver)) = text.split_once('/') else {
        return super::check_name(text);
    };
    for part in [package, driver] {
        match part.chars().next() {
            Some('a'..='z') => {}
            _ => return Err("each part must start with a lowercase letter"),
        }
        if !part
            .chars()
            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
        {
            return Err("only lowercase letters, digits, - and _ around one /");
        }
        if part.len() > 64 {
            return Err("at most 64 characters on each side of /");
        }
    }
    Ok(())
}
