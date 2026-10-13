//! `miyu kb` 印的样子（`docs/blueprint/cli/kb.md`「样子」）：纯的，测试里照它算。

use serde_json::Value;

use crate::language::Language;

/// `list`：一个库一行，编号照最长的对齐；后面是名字（和编号一样的不写）、几篇、开了自动想起的「自动想起」（施工 K-6 补）、
/// 说明（没写的不写），两个空格隔开；用不了的接上为什么。
pub(super) fn bases(bases: &[Value], language: &Language) -> Vec<String> {
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
