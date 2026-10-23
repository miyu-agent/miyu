//! 路径写给她看的样子（施工 X-10 下起在技能里，施工 P-14 修挪出来和项目指令共用）。

use std::path::Path;

/// 路径写给她看的样子：Windows 上反斜杠换成 `/`（转义会把反斜杠写成两个，她照抄就对不上；`/` 在 Windows 上一样认）。换真实
/// 位置时加上的 `\\?\` 去掉（施工 X-37：链接成的技能照真实的位置记）：`\\?\C:\x` 写成 `C:/x`，`\\?\UNC\host\share` 写成
/// `//host/share`；别的平台上没有这个前缀。
pub(crate) fn path_text(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = match (text.strip_prefix(r"\\?\UNC\"), text.strip_prefix(r"\\?\")) {
        (Some(rest), _) => format!(r"\\{rest}"),
        (None, Some(rest)) => rest.to_string(),
        (None, None) => text.into_owned(),
    };
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}
