//! 测试用的（`testkit` 特性，别的 crate 的测试打开；施工 TMP-1）。

use std::path::Path;

/// 删掉测试的临时目录 `dir`，删不掉的留着，不影响测试。删到一半，后台还在写的（回忆库、会话索引）往还在的那一层里添了
/// 文件，那一层删不掉、整个删失败：数据根的标记往往已经删了，再写的建不了新目录，隔 50 毫秒再删一次，最多五秒（施工 TMP-1
/// 补：门禁满载时一秒不够，偶尔剩一个）。删得掉的头一次就删掉，不多等。Windows 上只删一次：删不掉的是别处还开着的文件
/// （核心、后台任务拿着库的句柄），等多久都删不掉，一千多个夹具各等五秒，CI 超时。
pub fn remove_tree(dir: &Path) {
    let tries = if cfg!(windows) { 1 } else { 100 };
    for _ in 0..tries {
        if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
