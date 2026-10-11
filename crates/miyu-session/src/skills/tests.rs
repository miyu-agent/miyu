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
    }
}

fn skill(name: &str, description: &str) -> Skill {
    let dir = Path::new("/data/home/admin/skills").join(name);
    Skill {
        head: SkillHead {
            name: name.to_string(),
            description: description.to_string(),
            display_name: None,
            summary: None,
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
