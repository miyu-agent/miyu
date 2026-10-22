//! 项目指令（施工 P-14，`docs/blueprint/project-docs.md`）：仓库里的 `AGENTS.md`（没有的找 `CLAUDE.md`）回合开始作为一块事实
//! 交给她，不进 system（文件一变整段重算、缓存就断）。基础系统的功能 `project-docs`，预设能关；场所会话不读。
//!
//! - 找哪些：工作目录在 git 仓库里的，从仓库根（有 `.git` 的那一层）到工作目录逐层找，离工作目录越近的排得越后；不在仓库里的
//!   只找工作目录那一层（2026-10-23 项目主人定）。所有层共用一个总上限，超了截断、说一句。
//! - 什么时候交：这一段上下文里这个仓库没交过的、交过但内容变了的（换一份，头一句说替换哪个仓库的）。压缩、撤销拿走了的，
//!   下一轮照「没交过」再交。shellhook 的会话换仓库时第一次走进哪个就交哪个，同一个仓库里换子目录不重交。
//! - 内容是仓库作者写的字，原样放进外壳；兜底的是权限和沙盒（不设信任的门，照 Codex、opencode）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miyu_kernel::event::ContextInjected;
use miyu_kernel::facts::Present;
use miyu_kernel::id::{ContentHash, FactKind, ModuleId};
use miyu_kernel::session::Injection;
use miyu_kernel::template::Template;
use miyu_store::resources::ProjectDocTexts;

#[cfg(test)]
mod tests;

/// 模块编号、类别，也是基础系统里那个功能的编号。
pub(crate) const MODULE: &str = "project-docs";

/// 每一层先找的、没有再找的。
const NAMES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// 所有层加起来最多交多少字节（照 Codex 的默认）。
pub(crate) const LIMIT: usize = 32 * 1024;

/// 找到的：仓库根（不在仓库里的是工作目录），从外到里的几份，截没截。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub(crate) root: PathBuf,
    pub(crate) docs: Vec<(PathBuf, String)>,
    pub(crate) cut: bool,
}

/// 照工作目录 `cwd`（真实的路径）找一遍；一份都没有的没有。碰磁盘，在阻塞线程里调。
pub(crate) fn find(cwd: &Path) -> Option<Found> {
    let root = cwd
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(cwd)
        .to_path_buf();
    let mut layers: Vec<&Path> = cwd
        .ancestors()
        .take_while(|dir| dir.starts_with(&root))
        .collect();
    layers.reverse();
    let (mut docs, mut left, mut cut) = (Vec::new(), LIMIT, false);
    for dir in layers {
        let Some((path, text)) = NAMES.iter().find_map(|name| read(&dir.join(name))) else {
            continue;
        };
        if text.len() > left {
            docs.push((path, cut_at(&text, left).to_string()));
            cut = true;
            break;
        }
        left -= text.len();
        docs.push((path, text));
    }
    (!docs.is_empty()).then_some(Found { root, docs, cut })
}

/// 一份普通文件的字；不在、读不了、不是 UTF-8 的没有。
fn read(path: &Path) -> Option<(PathBuf, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    path.is_file().then(|| (path.to_path_buf(), text))
}

/// `text` 截到最多 `limit` 字节，不切开一个字。
fn cut_at(text: &str, limit: usize) -> &str {
    let mut end = limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// 这一轮要不要交：`present`（这一段上下文里模块注入过的）里这个仓库最后一块的指纹和这一刻的一样的不交；没交过的交，交过
/// 但变了的头一句说换一份。没找到的不交。
///
/// # Panics
///
/// 实际不会：出厂的字在读的时候就照模板查过（[`check`]）。
pub(crate) fn tail(
    texts: &ProjectDocTexts,
    found: Option<&Found>,
    present: &[Present],
) -> Option<Injection> {
    let found = found?;
    let root = found.root.display().to_string();
    let body = render(texts, found);
    let digest = ContentHash::of(body.as_bytes()).as_str().to_string();
    let last = present
        .iter()
        .rev()
        .find(|one| one.module.as_str() == MODULE && one.refs.get(1) == Some(&root));
    if last.and_then(|one| one.refs.first()) == Some(&digest) {
        return None;
    }
    let text = match last {
        Some(_) => fill(&texts.replaced, "root", &root) + &body,
        None => body,
    };
    Some(Injection {
        module: ModuleId::parse(MODULE).expect("合模块编号的写法"),
        fact: ContextInjected {
            kind: FactKind::parse(MODULE).expect("合类别的写法"),
            text,
            refs: vec![digest, root],
            after: false,
        },
    })
}

/// 读到的字照模板换得出来：每份要的字段都在。
///
/// # Errors
///
/// 哪一份写法坏了、要了没有的字段，交回原因。
pub(crate) fn check(texts: &ProjectDocTexts) -> Result<(), String> {
    for (template, field) in [
        (&texts.open, "path"),
        (&texts.replaced, "root"),
        (&texts.cut, "limit"),
        (&texts.close, "path"),
    ] {
        let parsed = Template::parse(template).map_err(|error| error.to_string())?;
        parsed
            .render(&BTreeMap::from([(field, "")]))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// 几份文件各包一层外壳，截了的末尾说一句。
fn render(texts: &ProjectDocTexts, found: &Found) -> String {
    let mut body = String::new();
    for (path, text) in &found.docs {
        body.push_str(&fill(&texts.open, "path", &path.display().to_string()));
        body.push_str(text);
        if !text.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&texts.close);
    }
    if found.cut {
        body.push_str(&fill(&texts.cut, "limit", &LIMIT.to_string()));
    }
    body
}

/// 模板 `template` 的字段 `field` 换成 `value`（照常转义）。
fn fill(template: &str, field: &str, value: &str) -> String {
    Template::parse(template)
        .and_then(|parsed| parsed.render(&BTreeMap::from([(field, value)])))
        .expect("读的时候查过")
}
