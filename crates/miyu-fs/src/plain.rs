//! 给人、给头看的路径（施工 WP-1）：Windows 上换真实位置（`canonicalize`）会加上扩展长度的前缀 `\\?\`，头拿它去列目录、人看到
//! 它都不认；核心自己用的照旧带着，交出去之前去掉。原来基础系统、端点各写了一份，挪到这里共用。

use std::path::Path;

/// 绝对路径去掉 Windows 换真实位置时加上的 `\\?\`：`\\?\C:\x` 写成 `C:\x`，`\\?\UNC\host\share` 写成
/// `\\host\share`。别的平台上不会有这个前缀，照原样。
pub fn plain(real: &Path) -> String {
    let text = real.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => text.into_owned(),
    }
}

#[cfg(test)]
mod tests;
