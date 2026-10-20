//! 技能那一块（施工 X-10 上）：一个技能一行，三格照模板转义；工具面上没有 `read` 的、没有技能的没有这一块。

use std::path::Path;

use miyu_kernel::tool::Access;
use miyu_policy::skill::SkillHead;

use super::*;

fn resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

fn tool(name: &str) -> ToolEntry {
    ToolEntry {
        name: name.to_string(),
        description: String::new(),
        parameters: serde_json::from_str("{}").expect("是 JSON"),
        access: Access::Read,
        need_load: false,
        clicall: false,
    }
}

fn skill(name: &str, description: &str) -> Skill {
    let dir = Path::new("/data/home/admin/skills").join(name);
    Skill {
        head: SkillHead {
            name: name.to_string(),
            description: description.to_string(),
            display_name: std::collections::BTreeMap::new(),
            summary: std::collections::BTreeMap::new(),
        },
        file: dir.join("SKILL.md"),
        dir,
        package: None,
    }
}

#[test]
fn each_skill_is_one_line_with_its_file() {
    let skills = [
        skill("notes", "Take notes. Use when asked to \"remember\"."),
        skill("pdf", "Read <b>PDF</b> files."),
    ];
    let block = listing(&resources(), &[tool("shell"), tool(READ)], &skills).expect("读得出");
    let file = |name: &str| {
        shown(
            &Path::new("/data/home/admin/skills")
                .join(name)
                .join("SKILL.md"),
        )
    };
    assert_eq!(
        block,
        Some(format!(
            "Skills you can use (read the file before following one):\n\
             - notes: Take notes. Use when asked to \\u0022remember\\u0022. ({})\n\
             - pdf: Read \\u003cb\\u003ePDF\\u003c/b\\u003e files. ({})",
            file("notes"),
            file("pdf")
        )),
        "说明照模板转义：伪造不了标签"
    );
    assert_eq!(
        shelves(&skills),
        [skills[0].dir.clone(), skills[1].dir.clone()]
    );
}

#[test]
fn no_read_tool_or_no_skill_lists_nothing() {
    let skills = [skill("notes", "Take notes.")];
    let listed =
        |face: &[ToolEntry], skills: &[Skill]| listing(&resources(), face, skills).expect("读得出");
    assert_eq!(listed(&[tool("shell")], &skills), None, "没有 read 读不了");
    assert_eq!(listed(&[tool(READ)], &[]), None);
}

/// 放开只读的文件夹换成真实的位置（施工 X-10 下）：经链接给的照它指到的地方，权限策略、沙盒、认用过的技能都照真实的位置比
/// （macOS 上临时目录、数据根常在 `/var` 这个链接后面）。
#[cfg(unix)]
#[test]
fn shelves_are_real_places() {
    let base = std::env::temp_dir().join(format!("miyu-skills-shelves-{}", std::process::id()));
    if base.exists() {
        std::fs::remove_dir_all(&base).expect("删得了");
    }
    let real = base.join("real").join("notes");
    std::fs::create_dir_all(&real).expect("建得了");
    std::os::unix::fs::symlink(base.join("real"), base.join("link")).expect("建得了链接");
    let mut via = skill("notes", "Take notes.");
    via.dir = base.join("link").join("notes");
    let found = shelves(&[via]);
    let wanted = std::fs::canonicalize(&real).expect("在");
    std::fs::remove_dir_all(&base).expect("删得了");
    assert_eq!(found, [wanted]);
}
