//! 技能的开头（施工 X-10 上）：公开格式的几种写法读得出，写错的说哪里不对。

use super::*;

fn skill(front: &str) -> String {
    format!("---\n{front}---\n\n# 正文\n")
}

#[test]
fn the_public_format_reads() {
    let text = skill(
        "name: pdf-processing\ndescription: Extract PDF text, fill forms. Use when handling PDFs.\nlicense: Apache-2.0\nmetadata:\n  author: example-org\n  version: \"1.0\"\n  display_name: PDF 处理\n  summary: 读 PDF、填表\n",
    );
    assert_eq!(
        read("pdf-processing", &text),
        Ok(SkillHead {
            name: "pdf-processing".to_string(),
            description: "Extract PDF text, fill forms. Use when handling PDFs.".to_string(),
            display_name: Some("PDF 处理".to_string()),
            summary: Some("读 PDF、填表".to_string()),
        })
    );
}

#[test]
fn quoted_and_folded_descriptions_read_too() {
    let quoted = skill("name: a\ndescription: \"Say: hi, \\\"you\\\"\"\n");
    assert_eq!(
        read("a", &quoted).map(|head| head.description),
        Ok("Say: hi, \"you\"".to_string())
    );
    let folded = skill("name: a\ndescription: >\n  First line\n  goes on.\n");
    assert_eq!(
        read("a", &folded).map(|head| head.description),
        Ok("First line goes on.".to_string())
    );
    let literal = skill("name: a\ndescription: |\n  First line.\n  Second\tline.\n");
    assert_eq!(
        read("a", &literal).map(|head| head.description),
        Ok("First line. Second line.".to_string()),
        "并成一行"
    );
    // Windows 上检出的带 `\r`、开头带 BOM 的照样读。
    let crlf = "\u{feff}---\r\nname: a\r\ndescription: Hi.\r\n---\r\nbody\r\n";
    assert_eq!(read("a", crlf).map(|head| head.name), Ok("a".to_string()));
}

#[test]
fn long_descriptions_are_cut_to_the_format_limit() {
    let long = "字".repeat(DESCRIPTION_CHARS + 10);
    let head = read("a", &skill(&format!("name: a\ndescription: {long}\n"))).expect("读得出");
    assert_eq!(head.description.chars().count(), DESCRIPTION_CHARS);
    assert_eq!((head.display_name, head.summary), (None, None));
}

#[test]
fn wrong_ones_say_what_is_wrong() {
    let cases = [
        ("a", "# 没有开头\n".to_string(), SkillProblem::NoFrontmatter),
        (
            "a",
            "---\nname: a\n".to_string(),
            SkillProblem::NoFrontmatter,
        ),
        ("a", skill("description: Hi.\n"), SkillProblem::NoName),
        (
            "a",
            skill("name: [a]\ndescription: Hi.\n"),
            SkillProblem::NoName,
        ),
        (
            "PDF",
            skill("name: PDF\ndescription: Hi.\n"),
            SkillProblem::BadName("PDF".to_string()),
        ),
        (
            "-a",
            skill("name: -a\ndescription: Hi.\n"),
            SkillProblem::BadName("-a".to_string()),
        ),
        (
            "a--b",
            skill("name: a--b\ndescription: Hi.\n"),
            SkillProblem::BadName("a--b".to_string()),
        ),
        (
            "b",
            skill("name: a\ndescription: Hi.\n"),
            SkillProblem::NameMismatch {
                name: "a".to_string(),
                dir: "b".to_string(),
            },
        ),
        ("a", skill("name: a\n"), SkillProblem::NoDescription),
        (
            "a",
            skill("name: a\ndescription: \"  \"\n"),
            SkillProblem::NoDescription,
        ),
    ];
    for (dir, text, wanted) in cases {
        assert_eq!(read(dir, &text), Err(wanted), "{text}");
    }
    assert!(matches!(
        read("a", &skill("name: a\ndescription: [unclosed\n")),
        Err(SkillProblem::Yaml(_))
    ));
    let long = "a".repeat(65);
    assert_eq!(
        read(&long, &skill(&format!("name: {long}\ndescription: Hi.\n"))),
        Err(SkillProblem::BadName(long.clone()))
    );
}
