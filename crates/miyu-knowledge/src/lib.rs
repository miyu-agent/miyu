//! 知识库这个软件包（施工 K-1 下，`docs/blueprint/knowledge.md`「她的工具」）：她用来查、写知识库的几件工具。不放进基础
//! 系统：知识库是可选、能关的软件包（`docs/designs/19-知识库.md` N10）。
//!
//! - [`kb_search`]：照关键词搜她查得到的库；
//! - [`kb_read`]：读一个文档的一页；
//! - [`kb_save`]：新建、追加一篇，写到还没有的库就是建这个库（施工 K-3，19 N14）；
//! - [`kb_edit`]：照原文改一段（施工 K-3）；
//! - [`kb_delete`]：把一篇挪进系统回收站（施工 K-3）。
//!
//! 写的三件报效果（改了文件、挪进回收站）：撤销那一轮时改回去、移回来，同 `write`、`trash`。
//!
//! 工具只拿 [`miyu_tool::KnowledgePort`]：知识库的目录、索引、查哪几个库都在端口后面（实现在 `miyu-session`）。给模型看的字在
//! 资源目录的 `software/knowledge/` 下（进登记簿），给人看的在 `software/knowledge/human/`。
//!
//! [`kb_search`]: miyu_tool::KB_SEARCH
//! [`kb_read`]: miyu_tool::KB_READ
//! [`kb_save`]: miyu_tool::KB_SAVE
//! [`kb_edit`]: miyu_tool::KB_EDIT
//! [`kb_delete`]: miyu_tool::KB_DELETE

mod delete;
mod edit;
mod read;
mod save;
mod search;
mod texts;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 这个软件包的编号：预设里开关它的那个键；资源目录另是 [`SOFTWARE`]（施工 PN-2）。
pub const PACKAGE: &str = "miyu-base-knowledge";

/// 资源目录 `resources/software/knowledge` 的名字，也是给人看的字的键 `software/knowledge/…` 的那一段（施工 PN-2）：日志里存着，
/// 和包编号 [`PACKAGE`] 分开，包改名不跟着变。
pub const SOFTWARE: &str = "knowledge";

/// 几件工具，照 `kb_search`、`kb_read`、`kb_save`、`kb_edit`、`kb_delete` 的先后。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let texts = texts::Texts::load(resources)?;
    Ok(vec![
        Arc::new(search::Search::load(resources, texts.clone())?),
        Arc::new(read::Read::load(resources, texts.clone())?),
        Arc::new(save::Save::load(resources, texts.clone())?),
        Arc::new(edit::Edit::load(resources, texts.clone())?),
        Arc::new(delete::Delete::load(resources, texts)?),
    ])
}
