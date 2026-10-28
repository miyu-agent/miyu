//! 测试用的（`testkit` 特性，别的 crate 的测试打开；施工 TMP-1）。

use std::path::Path;

/// 删掉测试的临时目录 `dir`，删不掉的留着，不影响测试。删到一半，后台还在写的（回忆库、会话索引）往还在的那一层里添了
/// 文件，那一层删不掉、整个删失败：数据根的标记往往已经删了，再写的建不了新目录，隔 50 毫秒再删一次，最多一秒。
pub fn remove_tree(dir: &Path) {
    for _ in 0..20 {
        if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
