//! 抽取、合并的指令里主人怎么写（施工 R-6 再修，`docs/blueprint/memory.md` 第六条第 4 款、第七条第 2 款；项目主人 2026-10-13、
//! 14 定：统一成称呼，没设的写「用户」）：指令里的 `{owner}` 换成称呼，没有的换成 `common/owner-unnamed.txt` 那一句。换进去的
//! 照模板的规矩转义成一行，所以那一句和称呼都不带引号（实测不带也照写）。
//!
//! 称呼照这一次的冻结配置读（`profile.name`，只写在个人设置里）：改了称呼，下一次抽取、合并就照新的。什么时候用由调的一方定：
//! 抽取看这个会话的快照带没带「关于你」，合并照属主的。

use miyu_kernel::template::Template;
use miyu_tool::load::say;

use crate::config::Turn;
use crate::settings::ProfileSettings;

/// 配置 `config` 里的称呼：去掉前后空白，空的当没有。
pub(crate) fn called(config: &Turn) -> Option<String> {
    let name = ProfileSettings::from(&config.resolved.values()).name?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// 指令里的 `{owner}` 换成什么：有称呼 `name` 的是称呼，没有的是 `unnamed` 那一句。
pub(super) fn owner(name: Option<&str>, unnamed: &Template) -> String {
    match name {
        Some(name) => name.to_string(),
        None => say(unnamed, &[]).trim_end().to_string(),
    }
}
