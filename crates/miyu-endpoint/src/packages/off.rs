//! 把某个包整个关掉（施工 U23 末上，项目主人 2026-10-24）：设置 `packages.<编号>.off`（`miyu_session::packages_off`）。看得到的包
//! （[`Core::packages_of`]）去掉关了的：技能、人格预设、命令模板、交给会话的「装了的包」、预设页都跟着没有；会话每一轮照活的配置把
//! 关了的包并进预设文件扣下的那几个（`miyu_policy::preset::PresetFile::switching_off`），工具面、技能下一轮就没有。施工 BS-1 上中起
//! 没有必需的包，哪个都关得掉（出厂的卸不掉，照它关）。包的账号（平台桥、替主人干活的扩展）不看这一格。

use std::collections::BTreeSet;

use crate::Core;

impl Core {
    /// 关了的包：照系统配置读（施工 SU-4 中起场所会话也在主人名下，一样照它）。
    pub(crate) fn packages_off(&self) -> BTreeSet<String> {
        let values = self.mine().config().resolved().values();
        miyu_session::packages_off::of(&values)
    }
}
