//! 工具说明的同名覆盖（施工 OV-1 上）：读、叠、换进工具面；写坏了的说为什么。

use super::*;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;

fn entry() -> ToolEntry {
    ToolEntry {
        name: "read".to_string(),
        description: "Read a file.".to_string(),
        parameters: serde_json::from_str::<RawJson>(
            r#"{"type":"object","properties":{"path":{"type":"string","description":"Path."},"limit":{"type":"integer"}}}"#,
        )
        .expect("读得懂"),
        access: Access::Read,
        clicall: false,
        runs_commands: false,
        command_only: false,
    }
}

#[test]
fn an_override_replaces_only_what_it_writes() {
    let mine = ToolOverride::parse("[parameters.path]\ndescription = \"Absolute path only.\"\n")
        .expect("读得懂");
    let system = ToolOverride::parse(
        "description = \"Read a text file.\"\n[parameters.path]\ndescription = \"Any path.\"\n",
    )
    .expect("读得懂");
    let merged = mine.over(system);
    let mut tool = entry();
    merged.apply(&mut tool).expect("换得进");
    assert_eq!(
        tool.description, "Read a text file.",
        "自己的没写的照系统的"
    );
    let schema: serde_json::Value = serde_json::from_str(tool.parameters.get()).unwrap();
    assert_eq!(
        schema["properties"]["path"]["description"], "Absolute path only.",
        "自己的压着系统的"
    );
    assert_eq!(
        schema["properties"]["limit"],
        serde_json::json!({"type": "integer"}),
        "别的参数不动"
    );
}

#[test]
fn without_parameter_descriptions_the_schema_keeps_its_bytes() {
    let mut tool = entry();
    let before = tool.parameters.get().to_string();
    ToolOverride::parse("description = \"Read it.\"\n")
        .expect("读得懂")
        .apply(&mut tool)
        .expect("换得进");
    assert_eq!(tool.parameters.get(), before);
}

#[test]
fn a_parameter_the_tool_lacks_is_refused_and_nothing_changes() {
    let mut tool = entry();
    let over =
        ToolOverride::parse("description = \"New.\"\n[parameters.nope]\ndescription = \"No.\"\n")
            .expect("读得懂");
    let why = over.apply(&mut tool).expect_err("没有这个参数");
    assert!(why.contains("nope"), "{why}");
    assert_eq!(tool.description, "Read a file.", "什么都不动");
}

#[test]
fn bad_files_say_why() {
    for (text, says) in [
        ("description = \"读一个文件。\"\n", "not English"),
        ("description = \"\"\n", "empty"),
        ("name = \"read2\"\n", "unknown key name"),
        ("description = 1\n", "must be a string"),
        (
            "[parameters.path]\nsummary = \"x\"\n",
            "unknown key parameters.path.summary",
        ),
        ("description = [\n", ""),
    ] {
        let why = ToolOverride::parse(text).expect_err(text);
        assert!(why.contains(says), "{text}: {why}");
    }
    let long = format!("description = \"{}\"\n", "a".repeat(2001));
    assert!(
        ToolOverride::parse(&long)
            .expect_err("太长")
            .contains("too long")
    );
}

#[test]
fn the_upper_layer_s_description_wins() {
    let mine = ToolOverride::parse("description = \"Mine.\"\n").expect("读得懂");
    let system = ToolOverride::parse("description = \"System.\"\n").expect("读得懂");
    assert_eq!(mine.over(system).description.as_deref(), Some("Mine."));
}
