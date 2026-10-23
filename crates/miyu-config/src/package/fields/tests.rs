//! 作者文档对清单格式的真相源（施工 Y-5 末上）：中英两份 `manifest` 里，每张给作者用的表都有一节，节里写着它认的每一格；文档
//! 里写的表清单都认。加了格、表没写进文档的，这里当场报红。能力表、MCP 服务的格也对作者文档（施工 X-48）。

use super::*;

/// 中文、英文两份作者文档。
const DOCS: [(&str, &str); 2] = [
    (
        "manifest.md",
        include_str!("../../../../../docs/authors/manifest.md"),
    ),
    (
        "manifest.en.md",
        include_str!("../../../../../docs/authors/manifest.en.md"),
    ),
];

/// 文档里的一节：标题那一行和下面的字，到下一个二级标题为止。
fn sections(doc: &str) -> Vec<(&str, &str)> {
    doc.split("\n## ")
        .skip(1)
        .map(|part| part.split_once('\n').unwrap_or((part, "")))
        .collect()
}

/// 标题里写着这张表：`[名字]`、`[[名字]]`、`[名字.` 开头的。
fn names(heading: &str, table: &str) -> bool {
    [
        format!("`[{table}]`"),
        format!("`[[{table}]]`"),
        format!("`[{table}."),
    ]
    .iter()
    .any(|form| heading.contains(form.as_str()))
}

#[test]
fn every_table_and_key_is_in_the_authors_docs() {
    let mut missing = Vec::new();
    for (file, doc) in DOCS {
        let found = sections(doc);
        for table in TABLES.iter().filter(|table| table.for_authors) {
            let Some((_, body)) = found.iter().find(|(heading, _)| names(heading, table.name))
            else {
                missing.push(format!("{file}：没有 [{}] 那一节", table.name));
                continue;
            };
            for key in table.keys.iter().flat_map(|keys| keys.iter()) {
                if !body.contains(&format!("`{key}`")) {
                    missing.push(format!("{file}：[{}] 那一节没写 `{key}`", table.name));
                }
            }
        }
        for (heading, _) in &found {
            let table = heading
                .trim_start_matches(['`', '['])
                .split([']', '.', '`'])
                .next()
                .unwrap_or_default();
            if heading.starts_with("`[") && !known_table(table) {
                missing.push(format!("{file}：写了清单不认的表 [{table}]"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// 接 MCP 服务的中英两份作者文档（施工 X-48）。
const MCP_DOCS: [(&str, &str); 2] = [
    ("mcp.md", include_str!("../../../../../docs/authors/mcp.md")),
    (
        "mcp.en.md",
        include_str!("../../../../../docs/authors/mcp.en.md"),
    ),
];

/// 扩展能要的每一项能力都写进了中英两份 `manifest`（施工 X-48）：能力表在代码里（`Capability::ALL`），加了一项没写文档的报红。
#[test]
fn every_capability_is_in_the_authors_docs() {
    let mut missing = Vec::new();
    for (file, doc) in DOCS {
        for name in crate::package::Capability::ALL.map(crate::package::Capability::as_str) {
            if !doc.contains(&format!("`{name}`")) {
                missing.push(format!("{file}：没写能力 `{name}`"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// 一个 MCP 服务的每一格都写进了中英两份 `mcp`（施工 X-48）：清单的 `[[mcp]]`、个人设置里的服务同一套格（[`MCP`]），作者照
/// `mcp.md` 写，那里少一格就报红。
#[test]
fn every_mcp_field_is_in_the_mcp_docs() {
    let mut missing = Vec::new();
    for (file, doc) in MCP_DOCS {
        for key in MCP {
            if !doc.contains(&format!("| `{key}` |")) {
                missing.push(format!("{file}：表里没有 `{key}`"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
