//! 技能的开头（施工 X-10 上）：公开格式的几种写法读得出，写错的说哪里不对。

use std::collections::BTreeMap;

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
            display_name: BTreeMap::from([("en".to_string(), "PDF 处理".to_string())]),
            summary: BTreeMap::from([("en".to_string(), "读 PDF、填表".to_string())]),
            warnings: Vec::new(),
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
    assert_eq!(head.warnings, [SkillWarning::DescriptionCut], "截了要说");
    assert_eq!(
        (head.display_name, head.summary),
        (BTreeMap::new(), BTreeMap::new())
    );
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
        // 名字没写、写错的照文件夹的名字用（施工 PT-2 上）：文件夹的名字也不合的才不收。
        ("A B", skill("description: Hi.\n"), SkillProblem::NoName),
        (
            "A B",
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

/// 技能算的功能的编号（施工 X-11 上）：`skill-<名字>`。
#[test]
fn a_skill_is_the_feature_skill_dash_name() {
    assert_eq!(feature_id("pdf-processing"), "skill-pdf-processing");
}

/// 给人看的照语言（施工 X-13 下）：不带后缀的是 `en`，`_zh`、`_ja` 各是那一种；空的不记。
#[test]
fn names_for_people_come_in_each_language() {
    let text = skill(
        "name: a\ndescription: A.\nmetadata:\n  display_name: Write a skill\n  display_name_zh: 写技能\n  display_name_ja: \"\"\n  summary_zh: 照包的样子写\n",
    );
    let head = read("a", &text).expect("读得成");
    assert_eq!(
        head.display_name,
        BTreeMap::from([
            ("en".to_string(), "Write a skill".to_string()),
            ("zh".to_string(), "写技能".to_string())
        ])
    );
    assert_eq!(
        head.summary,
        BTreeMap::from([("zh".to_string(), "照包的样子写".to_string())])
    );
}

/// 宽松地读（施工 PT-2 上，agentskills.io 接入指南「Lenient validation」）：别家写的技能常见的小毛病照收、记警告。
#[test]
fn small_slips_are_kept_with_warnings() {
    let mismatched = read("pdf-main", &skill("name: pdf\ndescription: Hi.\n")).expect("照收");
    assert_eq!(mismatched.name, "pdf", "照写的名字用");
    assert_eq!(
        mismatched.warnings,
        [SkillWarning::NameMismatch {
            name: "pdf".to_string(),
            dir: "pdf-main".to_string()
        }]
    );
    for (front, written) in [
        ("description: Hi.\n", None),
        ("name: PDF\ndescription: Hi.\n", Some("PDF")),
        ("name: [a]\ndescription: Hi.\n", None),
    ] {
        let head = read("a", &skill(front)).expect("照收");
        assert_eq!(head.name, "a", "照文件夹的名字用：{front}");
        assert_eq!(
            head.warnings,
            [SkillWarning::NameFromFolder {
                written: written.map(str::to_string),
                dir: "a".to_string()
            }],
            "{front}"
        );
    }
    let colon = read(
        "a",
        &skill("name: a\ndescription: Use this skill when: the user says \"PDF\" or a\\b\nlicense: MIT\n"),
    )
    .expect("宽松重读读得成");
    assert_eq!(
        colon.description, "Use this skill when: the user says \"PDF\" or a\\b",
        "整句原样"
    );
    assert_eq!(colon.warnings, [SkillWarning::LenientYaml]);
    // 只有读不懂的那几行改：同一段里已经加了引号的照原样读，不再套一层。
    let mixed = read(
        "a",
        &skill("name: a\ndescription: \"Say: hi\"\nlicense: MIT: or Apache\n"),
    )
    .expect("宽松重读读得成");
    assert_eq!(
        (mixed.description.as_str(), mixed.warnings.as_slice()),
        ("Say: hi", [SkillWarning::LenientYaml].as_slice())
    );
    // 说明的上限照格式：正好 1024 个字不说，多一个就说。
    for (count, cut) in [(DESCRIPTION_CHARS, false), (DESCRIPTION_CHARS + 1, true)] {
        let head = read(
            "a",
            &skill(&format!("name: a\ndescription: {}\n", "x".repeat(count))),
        )
        .expect("读得成");
        assert_eq!(
            head.warnings.contains(&SkillWarning::DescriptionCut),
            cut,
            "{count}"
        );
    }
    let odd = read(
        "a",
        &skill(&format!(
            "name: a\ndescription: Hi.\ncompatibility: {}\nmetadata:\n  version: 1.0\n",
            "x".repeat(COMPATIBILITY_CHARS + 1)
        )),
    )
    .expect("照收");
    assert_eq!(
        odd.warnings,
        [SkillWarning::Compatibility, SkillWarning::Metadata]
    );
    let fine = read(
        "a",
        &skill(
            "name: a\ndescription: Hi.\ncompatibility: Needs git\nmetadata:\n  version: \"1.0\"\n",
        ),
    )
    .expect("照收");
    assert!(fine.warnings.is_empty(), "{:?}", fine.warnings);
    let not_text =
        read("a", &skill("name: a\ndescription: Hi.\ncompatibility: 3\n")).expect("照收");
    assert_eq!(not_text.warnings, [SkillWarning::Compatibility]);
    assert_eq!(
        (
            SkillWarning::NameMismatch {
                name: "pdf".to_string(),
                dir: "x".to_string()
            }
            .code(),
            SkillWarning::NameMismatch {
                name: "pdf".to_string(),
                dir: "x".to_string()
            }
            .detail()
        ),
        ("skill_name_mismatch", "pdf".to_string())
    );
}
