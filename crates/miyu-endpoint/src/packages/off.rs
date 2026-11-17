//! 关着的包（施工 UX-1 补，项目主人 2026-11-17 定「关了就全没，人人有开关」）：每个包都有开关，记在 `extensions.json` 的 `on`
//! （`package.enable`、`package.disable`，设置页的「启用」同它）。关着的包它带来的一切都不在：看得到的包（[`Core::packages_of`]）
//! 去掉它，技能、脚本、命令模板、人格预设、知识库、给别的包带的内容、交给会话的「装了的包」、预设页都跟着没有；会话每一轮照这时
//! 的开关把它并进预设文件扣下的那几个（`miyu_policy::preset::PresetFile::switching_off`），开着的会话下一轮工具面、技能就没有。
//! 进程、服务由开关自己停。出厂的内置包照旧是卸掉、装回（`switch.rs`），不记在这里。原来施工 U23 末上的配置项
//! `packages.<编号>.off`（只对某个人隐藏）删了。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_store::packages::Found;

use crate::Core;

impl Core {
    /// 关着的包，照编号排：装着的、清单读成了的里开关是关的（[`crate::extensions::package_on`]）。
    pub(crate) fn packages_off(&self) -> BTreeSet<String> {
        let (switches, _) = crate::extensions::read_switches(self);
        self.system_packages()
            .iter()
            .filter_map(|found| Some((found, found.read.as_ref().ok()?)))
            .filter(|(found, manifest)| {
                !crate::extensions::package_on(&switches, &found.id, manifest)
            })
            .map(|(found, _)| found.id.clone())
            .collect()
    }

    /// 开关变了以后（施工 UX-1 补）：知识库、本机的向量模型照开着的包重算，给别的包带的内容推一次 `content.changed`。技能、脚本、
    /// 命令、预设是用到时照 [`Core::packages_of`] 现算的，不用另管。
    pub(crate) async fn follow_switches(self: &Arc<Self>) {
        let visible = self.packages_of();
        let now: Vec<&Found> = visible.iter().collect();
        crate::knowledge::follow(&self.knowledge, &now);
        if let (Some(builtins), Some(vectors)) = (&self.builtins, self.memory.vectors()) {
            vectors.replace_local(builtins.embed(&now)).await;
        }
        self.content_rereads.bump();
    }
}
