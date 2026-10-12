//! 她查知识库（施工 K-1 下，`docs/blueprint/knowledge.md`「怎么走」第六条）：查得到哪几个库、知识库的端口；她的 system 里
//! 列出来的那一块（施工 K-1 补，「列出来的那一块」）；她写知识库在 `knowledge/write.rs`（施工 K-3，第七条）。
//!
//! 查记忆的账号下的全部库（`memory_account`：人格住在谁家，无人格的照属主，系统账号的照管理员），人格的 `persona.toml` 写了
//! `[knowledge] bases` 的只要列着的、在的那几个（19 N11）。照这一刻的目录和人格文件算：建了库、改了人格，下一次就照新的。
//! 一个都查不到的会话工具面上没有两件工具（2026-10-12 项目主人定），回合开始照 [`Shelves::listed`] 再判一次。碰磁盘的在阻塞
//! 线程里做。

mod write;

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_policy::preset::KNOWLEDGE;
use miyu_store::knowledge::{Knowledge, KnowledgeError, Skipped};
use miyu_store::personas::{PersonaError, Personas};
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_tool::load::{self, say};
use miyu_tool::{
    KB_HITS, KbHit, KbPage, KbRefused, KbSave, KbTrashed, KbWritten, KnowledgePort, Pending,
};

use crate::TARGET;
use crate::blocking::blocking;

/// system 里最多列几个库（施工 K-1 补）：多的末尾一句说还有几个，`kb_search` 照样搜得到。免得库多了 system 一直涨。
const LISTED: usize = 20;

/// 她查得到的一个库，列出来的样子：编号、名字（没写的照编号）、一句说明（没写的空着）。
pub(crate) struct Listed {
    id: String,
    name: String,
    description: String,
}

/// 一个会话查知识库要的：核心一份的知识库、照哪个账号、哪个人格；挪进回收站照哪个家目录（施工 K-3）。
pub(crate) struct Shelves {
    knowledge: Arc<Knowledge>,
    account: AccountId,
    persona: Option<String>,
    personas: Personas,
    home: Option<PathBuf>,
}

impl Shelves {
    /// 会话的知识库：照记忆的账号 `account`、人格 `persona`（几层是 `personas`）挑；删的挪进 `home`（这台机器的家目录，
    /// Linux 照它找回收站）的回收站。
    pub(crate) fn new(
        knowledge: Arc<Knowledge>,
        account: AccountId,
        persona: Option<String>,
        personas: Personas,
        home: Option<PathBuf>,
    ) -> Shelves {
        Shelves {
            knowledge,
            account,
            persona,
            personas,
            home,
        }
    }

    /// 人格收窄成哪几个（`[knowledge] bases`）：没人格的、没写的、人格的文件没了的（会话照快照接着用）都是没有。
    ///
    /// # Errors
    ///
    /// 人格的文件读不了：为什么。
    fn narrowing(&self) -> Result<Option<Vec<String>>, String> {
        let Some(persona) = &self.persona else {
            return Ok(None);
        };
        match self.personas.find(persona) {
            Ok(found) => Ok(found.file.knowledge),
            Err(PersonaError::NotFound(_)) => Ok(None),
            Err(error) => Err(error.to_string()),
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
        Ok(match self.narrowing()? {
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

    /// 她这一刻查得到的库，带着名字、说明（施工 K-1 补），先后同 [`Shelves::bases`]。工具面上给不给两件工具照它是不是空的
    /// （施工 K-1 下）。读不了的当一个都没有、记一行：下一个回合再判。
    pub(crate) fn listed(&self) -> Vec<Listed> {
        let listed = self.bases().and_then(|bases| {
            bases
                .into_iter()
                .map(|id| {
                    let (name, description) = self
                        .knowledge
                        .title(&self.account, &id)
                        .map_err(|error| error.to_string())?;
                    Ok(Listed {
                        id,
                        name,
                        description,
                    })
                })
                .collect::<Result<Vec<Listed>, String>>()
        });
        listed.unwrap_or_else(|error| {
            tracing::warn!(target: TARGET, error = error.as_str(), "knowledge unreadable");
            Vec::new()
        })
    }
}

/// 她的 system 里列出来的那一块（施工 K-1 补，`knowledge.md`「列出来的那一块」）：头一行，一个库一行（有说明的
/// `- 编号: 名字 — 说明`，没说明的 `- 编号: 名字`，名字就是编号的 `- 编号`），最多 [`LISTED`] 个，多的末尾一句。字从资源目录
/// 的 `software/knowledge/list/` 读，名字、说明照模板转义。一个都没有的没有这一块。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对：安装坏了，照随核心附带的字读不出来报。
pub(crate) fn listing(
    resources: &ResourceRoot,
    listed: &[Listed],
) -> Result<Option<String>, SourceError> {
    if listed.is_empty() {
        return Ok(None);
    }
    let text = |name: &str, fields: &[&str]| {
        load::text(resources.path(), KNOWLEDGE, "list", name, fields).map_err(|error| {
            SourceError::Read {
                path: error.file,
                error: std::io::Error::new(std::io::ErrorKind::InvalidData, error.why),
            }
        })
    };
    let line = |template: &miyu_kernel::template::Template, fields: &[(&str, &str)]| {
        say(template, fields).trim_end().to_string()
    };
    let (base, named, bare) = (
        text("base", &["id", "name", "description"])?,
        text("named", &["id", "name"])?,
        text("bare", &["id"])?,
    );
    let mut lines = vec![line(&text("open", &[])?, &[])];
    for one in listed.iter().take(LISTED) {
        lines.push(match (one.name == one.id, one.description.is_empty()) {
            (_, false) => line(
                &base,
                &[
                    ("id", &one.id),
                    ("name", &one.name),
                    ("description", &one.description),
                ],
            ),
            (false, true) => line(&named, &[("id", &one.id), ("name", &one.name)]),
            (true, true) => line(&bare, &[("id", &one.id)]),
        });
    }
    if listed.len() > LISTED {
        let count = (listed.len() - LISTED).to_string();
        lines.push(line(&text("more", &["count"])?, &[("count", &count)]));
    }
    Ok(Some(lines.join("\n")))
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

    fn save<'a>(&'a self, save: KbSave) -> Pending<'a, Result<KbWritten, KbRefused>> {
        let shelves = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || write::save(&shelves, save)).await })
    }

    fn edit<'a>(
        &'a self,
        kb: String,
        path: String,
        old: String,
        new: String,
    ) -> Pending<'a, Result<KbWritten, KbRefused>> {
        let shelves = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || write::edit(&shelves, kb, path, &old, &new)).await })
    }

    fn delete<'a>(&'a self, kb: String, path: String) -> Pending<'a, Result<KbTrashed, KbRefused>> {
        let shelves = Arc::clone(&self.0);
        Box::pin(async move { blocking(move || write::delete(&shelves, kb, path)).await })
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
        Err(KnowledgeError::BadPage) => Err(KbRefused::NoPage(page)),
        Err(error) => Err(refused(error, kb, path)),
    }
}

/// 知识库那一层的错写成她看的拒绝：库 `kb` 里的 `path` 那一篇。读不了、写不进的照实说，记一行。
fn refused(error: KnowledgeError, kb: String, path: String) -> KbRefused {
    match error {
        KnowledgeError::UnknownBase => KbRefused::NoBase(kb),
        KnowledgeError::UnknownDocument => KbRefused::NoDocument(kb, path),
        KnowledgeError::Exists => KbRefused::Exists(kb, path),
        KnowledgeError::TooLarge => KbRefused::TooLarge(path),
        KnowledgeError::NoMatch => KbRefused::NoMatch(path),
        KnowledgeError::Ambiguous(count) => KbRefused::Ambiguous(path, count),
        KnowledgeError::Trash(why) => KbRefused::Trash(path, why),
        error => failed(&error),
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
