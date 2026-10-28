//! 会话中途技能列表变了（施工 X-11 下，`docs/blueprint/skills.md`「定了的」第 2 条）：回合开始照这一刻装了的软件包、属主家目录、
//! 会话的预设再找一遍，放开只读的文件夹换成这一份；列出来的和她最后看到的不一样的，在这一轮的尾巴上交一份新的完整列表
//! （`context.injected`，模块 [`MODULE`]），system 里那一块不改，前缀缓存不掉（旧版 Miyu、dsh 都这样）。

use std::path::PathBuf;

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{ContentHash, FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_policy::ToolEntry;
use miyu_policy::preset::PresetFile;
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_store::root::DataRoot;

use super::{find, listing, opened, shelves};
use crate::open::Installed;

/// 技能那一块的模块编号和类别：`present` 里照它认出她最后看到的那一份。
pub(crate) const MODULE: &str = "skills";

/// 回合开始再找一遍要的：一个会话一份，造会话、载入时交给人格刷新那一份（`actor::persona::Refresh`）。
#[derive(Clone)]
pub(crate) struct Watch {
    /// 这一刻装了的软件包。
    pub(crate) installed: Installed,
    /// 数据根。
    pub(crate) root: DataRoot,
}

/// 这一刻找到的：放开只读的文件夹（真实的位置）、它们给人看的名字（施工 9-8 五补）、列出来的那一块（工具面上没有 `read`
/// 的、一个都没有的没有）。
pub(crate) struct Looked {
    pub(crate) shelves: Vec<PathBuf>,
    pub(crate) names: super::Names,
    pub(crate) listing: Option<String>,
}

impl Watch {
    /// 照会话现在的预设 `preset`、工具面 `face` 找一遍。碰磁盘，在阻塞线程里调。
    ///
    /// # Errors
    ///
    /// 列出来的那一块的字读不出来、写法不对。
    pub(crate) fn look(
        &self,
        resources: &ResourceRoot,
        face: &[ToolEntry],
        preset: Option<&PresetFile>,
    ) -> Result<Looked, SourceError> {
        let found = opened(find(&(self.installed)(), &self.root), preset);
        Ok(Looked {
            shelves: shelves(&found),
            names: super::names(&found),
            listing: listing(resources, face, &found)?,
        })
    }
}

/// 这一轮要不要交一份新的：她最后看到的是 `present`（这一段上下文里模块注入过的）里最后一块技能带的指纹，没有就是快照
/// system 里那一块 `pinned` 的；和这一刻的 `listing` 一样的不交。交的是外壳里的完整列表，一个都没有了的说一句。压缩以后
/// 前面交过的不在 `present` 里了，照快照里那一块比：变过的再交一次。
///
/// # Errors
///
/// 外壳的字读不出来。
pub(crate) fn tail(
    resources: &ResourceRoot,
    listing: Option<&str>,
    pinned: Option<&str>,
    present: &[Present],
) -> Result<Option<Injection>, SourceError> {
    let now = digest(listing);
    let last = present
        .iter()
        .rev()
        .find(|one| one.module.as_str() == MODULE && one.kind.as_str() == MODULE)
        .and_then(|one| one.refs.first().cloned())
        .unwrap_or_else(|| digest(pinned));
    if last == now {
        return Ok(None);
    }
    let texts = resources.skill_texts()?;
    let body = listing.unwrap_or(texts.none.trim_end());
    let text = format!("{}{body}\n{}", texts.changed_open, texts.changed_close);
    Ok(Some(Injection {
        module: ModuleId::parse(MODULE).expect("合模块编号的写法"),
        fact: ContextInjected {
            kind: FactKind::parse(MODULE).expect("合类别的写法"),
            text,
            refs: vec![now],
            after: false,
        },
    }))
}

/// 列出来的那一块的指纹：没有的照空的算。
fn digest(listing: Option<&str>) -> String {
    ContentHash::of(listing.unwrap_or_default().as_bytes())
        .as_str()
        .to_string()
}
