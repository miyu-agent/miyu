//! `miyu kb` 印的样子（`docs/blueprint/cli/kb.md`「样子」）：纯的，测试里照它算。

use std::collections::BTreeMap;

use serde_json::Value;

use crate::language::Language;

/// `list`：一个库一行，编号照最长的对齐；后面是名字（和编号一样的不写）、几篇、软件包带来的「随 <包名>」（施工 K-8，包名照
/// `names`，没有的照包的编号）、开了自动想起的「自动想起」（施工 K-6 补）、说明（没写的不写），两个空格隔开；用不了的接上
/// 为什么。
pub(super) fn bases(
    bases: &[Value],
    names: &BTreeMap<String, String>,
    language: &Language,
) -> Vec<String> {
    let text = |base: &Value, field: &str| base[field].as_str().unwrap_or_default().to_string();
    let width = bases
        .iter()
        .map(|base| text(base, "id").chars().count())
        .max()
        .unwrap_or(0);
    bases
        .iter()
        .map(|base| {
            let id = text(base, "id");
            let mut parts = vec![format!("{id:<width$}")];
            let name = text(base, "name");
            if name != id && !name.is_empty() {
                parts.push(name);
            }
            parts.push(language.kb_documents(base["documents"].as_u64().unwrap_or(0)));
            if let Some(package) = base["package"].as_str() {
                let name = names.get(package).map_or(package, String::as_str);
                parts.push(language.kb_from(name));
            }
            if base["auto"].as_bool() == Some(true) {
                parts.push(language.kb_auto().to_string());
            }
            let description = text(base, "description");
            if !description.is_empty() {
                parts.push(description);
            }
            let mut row = parts.join("  ");
            if let Some(error) = base["error"].as_str() {
                row.push_str(&language.kb_unavailable(error));
            }
            row
        })
        .collect()
}

/// `list <库>`（施工 K-10）：一篇一行，路径照最长的对齐，后面是大小、改的日期，两个空格隔开；这一回读不了的接上为什么。读不了的
/// 子目录一个一行接在后面（路径带 `/`）。块数不印（给人看的只放人要的）。
pub(super) fn documents(
    documents: &[Value],
    folders: &[Value],
    language: &Language,
) -> Vec<String> {
    let path = |one: &Value| one["path"].as_str().unwrap_or_default().to_string();
    let folder = |one: &Value| format!("{}/", path(one));
    let width = documents
        .iter()
        .map(path)
        .chain(folders.iter().map(folder))
        .map(|path| path.chars().count())
        .max()
        .unwrap_or(0);
    let unread = |one: &Value| {
        one["reason"]
            .as_str()
            .map(|reason| language.kb_unread(reason))
            .unwrap_or_default()
    };
    let rows = documents.iter().map(|one| {
        let at = one["at"].as_str().unwrap_or_default();
        let size = crate::pkg::size(one["size"].as_u64().unwrap_or(0));
        let date = at.get(..10).unwrap_or(at);
        format!("{:<width$}  {size}  {date}{}", path(one), unread(one))
            .trim_end()
            .to_string()
    });
    let folders = folders
        .iter()
        .map(|one| format!("{:<width$}{}", folder(one), unread(one)));
    rows.chain(folders).collect()
}

/// `search`：一块一段，段和段之间空一行：头一行是库、文档、标题路径（PDF 的是第几页，没有的不写），下面是这一块的字，每行
/// 空两格。
pub(super) fn hits(hits: &[Value], language: &Language) -> String {
    hits.iter()
        .map(|hit| {
            let field = |name: &str| hit[name].as_str().unwrap_or_default();
            let mut head = vec![field("kb").to_string(), field("path").to_string()];
            match (hit["page"].as_u64(), field("heading")) {
                (Some(page), _) => head.push(language.kb_pdf_page(page)),
                (None, "") => {}
                (None, heading) => head.push(heading.to_string()),
            }
            let body: Vec<String> = field("text")
                .trim_end()
                .lines()
                .map(|line| format!("  {line}").trim_end().to_string())
                .collect();
            std::iter::once(head.join("  "))
                .chain(body)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `read`：那一页的字（末尾的换行去掉），不止一页的另起一行说第几页。
pub(super) fn page(result: &Value, language: &Language) -> String {
    let text = result["text"].as_str().unwrap_or_default().trim_end();
    let (page, pages) = (
        result["page"].as_u64().unwrap_or(1),
        result["pages"].as_u64().unwrap_or(1),
    );
    match pages > 1 {
        true => format!("{text}\n{}", language.kb_page_of(page, pages)),
        false => text.to_string(),
    }
}
