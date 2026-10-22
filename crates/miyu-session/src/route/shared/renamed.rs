//! 供应商改名（施工 8-34，`docs/blueprint/models.md`「供应商改名」）：用出来的、供应商的列表跟着换编号，改名记进
//! `renamed.json`，钉着旧编号的会话每轮照它换（`route/turn.rs`）。

use miyu_models::observed::Renamed;

use super::{ModelData, TARGET};

impl ModelData {
    /// 旧编号 `from` 现在叫什么；没改过名的没有。
    pub fn renamed(&self, from: &str) -> Option<String> {
        self.lock().renamed.current(from).map(str::to_string)
    }

    /// `from` 改成了 `to`：学到的、列表换上新编号，改名记下来，写盘（在阻塞线程里调）。写不成的记一行，
    /// 内存里照样换。
    pub fn rename_provider(&self, from: &str, to: &str) {
        let mut observed = self.lock();
        observed.learned.rename_provider(from, to);
        let list = observed.lists.remove(from);
        if let Some(list) = &list {
            observed.lists.insert(to.to_string(), list.clone());
        }
        observed.renamed.record(from, to);
        let (learned, renamed) = (observed.learned.to_json(), observed.renamed.to_json());
        drop(observed);
        tracing::info!(target: TARGET, "provider renamed");
        self.write("learned.json", &learned);
        self.write("renamed.json", &renamed);
        if let Some(list) = list {
            self.write(&format!("providers/{to}.json"), &list.to_json());
            if let Some(dir) = &self.dir {
                // 没有的不要紧：本来就是派生的
                let _gone =
                    std::fs::remove_file(dir.join("providers").join(format!("{from}.json")));
            }
        }
    }
}

/// 读 `renamed.json`：没有的、坏的当没有（坏的记一行）。
pub(super) fn read(text: Option<String>) -> Renamed {
    text.and_then(|text| super::warn_if_broken(Renamed::parse(&text)))
        .unwrap_or_default()
}
