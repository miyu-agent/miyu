//! 脚本拿到的环境变量都写进了作者文档（施工 X-47）：中英两份 `docs/authors/scripts-skills` 里每一个都有。加了变量没写文档的，这里
//! 当场报红。
//!
//! 不写给作者的（内部用的，不进这张表）：`MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN`（`miyu call` 自己读，作者照 `miyu call` 的用法写就行，
//! `shell/channel`）；`MIYU_COMMAND`（Windows 上起命令的那一层自己用，`shell/program.rs`）。

use super::*;

/// 中文、英文两份作者文档。
const DOCS: [(&str, &str); 2] = [
    (
        "scripts-skills.md",
        include_str!("../../../../../docs/authors/scripts-skills.md"),
    ),
    (
        "scripts-skills.en.md",
        include_str!("../../../../../docs/authors/scripts-skills.en.md"),
    ),
];

/// 脚本拿得到的每一个：参数、几个目录、调用的事实、模型和日志、家目录那几处、包自己的配置项（施工 PS-1）。
fn given() -> Vec<&'static str> {
    let mut names = vec![
        ARGS,
        CACHE,
        IMAGES,
        STATE,
        CALLER_VENUE,
        CALLER_OWNER,
        SETTINGS,
        super::super::env::MODEL,
        super::super::env::EFFORT,
        super::super::env::SESSION_LOG,
    ];
    names.extend(
        miyu_store::root::USER_DIRS
            .iter()
            .filter_map(|name| home::var(name)),
    );
    names
}

#[test]
fn every_variable_a_script_gets_is_in_the_authors_docs() {
    let mut missing = Vec::new();
    for (file, doc) in DOCS {
        for name in given() {
            if !doc.contains(&format!("`{name}`")) {
                missing.push(format!("{file}：没写 `{name}`"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
