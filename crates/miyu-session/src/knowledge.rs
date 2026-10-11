//! 她查知识库（施工 K-1 下，`docs/blueprint/knowledge.md`「怎么走」第六条）：查得到哪几个库、知识库的端口。
//!
//! 查记忆的账号下的全部库（`memory_account`：人格住在谁家，无人格的照属主，系统账号的照管理员），人格的 `persona.toml` 写了
//! `[knowledge] bases` 的只要列着的、在的那几个（19 N11）。照这一刻的目录和人格文件算：建了库、改了人格，下一次就照新的。
//! 一个都查不到的会话工具面上没有两件工具（2026-10-12 项目主人定），回合开始照 [`Shelves::any`] 再判一次。碰磁盘的在阻塞
//! 线程里做。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_store::knowledge::{Knowledge, KnowledgeError, Skipped};
use miyu_store::personas::{PersonaError, Personas};
use miyu_tool::{KB_HITS, KbHit, KbPage, KbRefused, KnowledgePort, Pending};

use crate::TARGET;
use crate::blocking::blocking;

/// 一个会话查知识库要的：核心一份的知识库、照哪个账号、哪个人格。
pub(crate) struct Shelves {
    knowledge: Arc<Knowledge>,
    account: AccountId,
    persona: Option<String>,
    personas: Personas,
}

impl Shelves {
    /// 会话的知识库：照记忆的账号 `account`、人格 `persona`（几层是 `personas`）挑。
    pub(crate) fn new(
        knowledge: Arc<Knowledge>,
        account: AccountId,
        persona: Option<String>,
        personas: Personas,
    ) -> Shelves {
        Shelves {
            knowledge,
            account,
            persona,
            personas,
        }
    }

    /// 她这一刻查得到的库：账号下在的，照编号排；人格收窄的只留列着的、在的那几个，照列的先后。人格的文件没了的（会话照
    /// 快照接着用）当没收窄。
    ///
    /// # Errors
    ///
    /// 知识库的目录、人格的文件读不了：为什么。
    pub(crate) fn bases(&self) -> Result<Vec<String>, String> {
        let ids = self
            .knowledge
            .ids(&self.account)
            .map_err(|error| error.to_string())?;
        let Some(persona) = &self.persona else {
            return Ok(ids);
        };
        let narrowed = match self.personas.find(persona) {
            Ok(found) => found.file.knowledge,
            Err(PersonaError::NotFound(_)) => None,
            Err(error) => return Err(error.to_string()),
        };
        Ok(match narrowed {
            Some(listed) => {
                let mut kept: Vec<String> = Vec::new();
                for id in listed {
                    if ids.contains(&id) && !kept.contains(&id) {
                        kept.push(id);
                    }
                }
                kept
            }
            None => ids,
        })
    }

    /// 查得到至少一个库：工具面上给不给两件工具照它。读不了的当没有、记一行：下一个回合再判。
    pub(crate) fn any(&self) -> bool {
        match self.bases() {
            Ok(bases) => !bases.is_empty(),
            Err(error) => {
                tracing::warn!(target: TARGET, error = error.as_str(), "knowledge unreadable");
                false
            }
        }
    }
}

/// 交给两件工具的端口：一个会话一份，每次调用照这一刻算查得到哪几个。
pub(crate) fn port(shelves: &Arc<Shelves>) -> Arc<dyn KnowledgePort> {
    Arc::new(Port(Arc::clone(shelves)))
}

struct Port(Arc<Shelves>);

impl KnowledgePort for Port {
    fn search<'a>(
        &'a self,
        query: String,
        kb: Option<String>,
    ) -> Pending<'a, Result<Vec<KbHit>, KbRefused>> {
        let shelves = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || search(&shelves, &query, kb)).await })
    }

    fn read<'a>(
        &'a self,
        kb: String,
        path: String,
        page: usize,
    ) -> Pending<'a, Result<KbPage, KbRefused>> {
        let shelves = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || read(&shelves, kb, path, page)).await })
    }
}

/// 搜：写了 `kb` 的只搜它（她查不到的当没有），没写的搜她查得到的全部。
fn search(shelves: &Shelves, query: &str, kb: Option<String>) -> Result<Vec<KbHit>, KbRefused> {
    let bases = shelves.bases().map_err(KbRefused::Failed)?;
    let only = match kb {
        Some(kb) if bases.contains(&kb) => vec![kb],
        Some(kb) => return Err(KbRefused::NoBase(kb)),
        None => bases,
    };
    let (hits, skipped) = shelves
        .knowledge
        .search(&shelves.account, query, Some(&only), KB_HITS)
        .map_err(|error| failed(&error))?;
    report(&skipped);
    Ok(hits
        .into_iter()
        .map(|hit| KbHit {
            kb: hit.kb,
            path: hit.path,
            heading: hit.heading,
            text: hit.text,
        })
        .collect())
}

/// 读一页：库得是她查得到的。
fn read(shelves: &Shelves, kb: String, path: String, page: usize) -> Result<KbPage, KbRefused> {
    let bases = shelves.bases().map_err(KbRefused::Failed)?;
    if !bases.contains(&kb) {
        return Err(KbRefused::NoBase(kb));
    }
    match shelves.knowledge.read(&shelves.account, &kb, &path, page) {
        Ok(found) => Ok(KbPage {
            text: found.text,
            page: found.page,
            pages: found.pages,
        }),
        Err(KnowledgeError::UnknownBase) => Err(KbRefused::NoBase(kb)),
        Err(KnowledgeError::UnknownDocument) => Err(KbRefused::NoDocument(kb, path)),
        Err(KnowledgeError::BadPage) => Err(KbRefused::NoPage(page)),
        Err(error) => Err(failed(&error)),
    }
}

/// 读不了：照实说，记一行。
fn failed(error: &KnowledgeError) -> KbRefused {
    tracing::warn!(target: TARGET, error = %error, "knowledge failed");
    KbRefused::Failed(error.to_string())
}

/// 跳过的文档记一行（`knowledge.md`「怎么走」第三条第 4 款）。
fn report(skipped: &[Skipped]) {
    for one in skipped {
        tracing::warn!(target: TARGET, kb = one.kb.as_str(), path = one.path.as_str(), why = one.why.as_str(), "knowledge document skipped");
    }
}
