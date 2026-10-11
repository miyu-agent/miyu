//! 知识库这个软件包（施工 K-1 下，`docs/blueprint/knowledge.md`「她的工具」）：她用来查知识库的两件工具。不放进基础系统：
//! 知识库是可选、能关的软件包（`docs/designs/19-知识库.md` N10）。
//!
//! - [`kb_search`]：照关键词搜她查得到的库；
//! - [`kb_read`]：读一个文档的一页。
//!
//! 工具只拿 [`miyu_tool::KnowledgePort`]：知识库的目录、索引、查哪几个库都在端口后面（实现在 `miyu-session`）。给模型看的字在
//! 资源目录的 `software/knowledge/` 下（进登记簿），给人看的在 `software/knowledge/human/`。
//!
//! [`kb_search`]: miyu_tool::KB_SEARCH
//! [`kb_read`]: miyu_tool::KB_READ

mod read;
mod search;
mod texts;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 这个软件包的编号：资源目录里的名字，也是预设里开关它的那个键。
pub const PACKAGE: &str = "knowledge";

/// 两件工具，照 `kb_search`、`kb_read` 的先后。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let texts = texts::Texts::load(resources)?;
    Ok(vec![
        Arc::new(search::Search::load(resources, texts.clone())?),
        Arc::new(read::Read::load(resources, texts)?),
    ])
}
