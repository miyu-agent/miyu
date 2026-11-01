//! 写的是路径还是编号（施工 F-8 上起；IN-4 从 `pkg.rs` 挪出来，`install`、`info` 共用一个判法）：相对的照现在的目录换成
//! 绝对的（核心只收绝对的）。

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

/// 写的是一个包目录（或它里面的清单）、包文件的路径：带 `/`、`\`，是 `.`、`..`，或以 `.toml`、`.miyu` 结尾的。别的是编号。
pub(super) fn is_path(what: &str) -> bool {
    what.contains('/')
        || what.contains('\\')
        || what == "."
        || what == ".."
        || what.ends_with(".toml")
        || what.ends_with(".miyu")
}

/// `install` 装的是什么：路径（[`is_path`]）换成绝对的（相对的照 `cwd` 算，施工 F-8 上）；别的是卸掉了的出厂的包的编号。
pub(super) fn installing(what: &str, cwd: &Path) -> Value {
    match is_path(what) {
        true => json!({"path": joined(cwd, what)}),
        false => json!({"package": what}),
    }
}

/// 相对的照 `cwd` 接成绝对的，`.`、`..` 照字面去掉（不碰磁盘）：核心照最后一段认编号，`.` 认不出来。
pub(super) fn joined(cwd: &Path, what: &str) -> PathBuf {
    let mut path = PathBuf::new();
    for part in cwd.join(what).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                path.pop();
            }
            other => path.push(other),
        }
    }
    path
}
