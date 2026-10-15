//! 工具面上要先加载的（施工 L-1）：参数换成外壳、加上 `load_tools` 照名字排进去、规则留着完整格式；没有要先加载的、快照里
//! 没有那几句字的一个字节都不动。

use miyu_kernel::tool::Access;

use super::*;

fn entry(name: &str, need_load: bool) -> ToolEntry {
    ToolEntry {
        name: name.to_string(),
        description: format!("{name} does it."),
        parameters: serde_json::from_str(
            r#"{"type":"object","properties":{"q":{"type":"string"}}}"#,
        )
        .unwrap(),
        access: Access::Read,
        need_load,
    }
}

fn texts() -> LoadingTexts {
    LoadingTexts {
        description: "Load tools.".to_string(),
        parameters: serde_json::from_str(
            r#"{"type":"object","properties":{"tools":{"type":"array"}}}"#,
        )
        .unwrap(),
        unknown: "None for {names}.\n".to_string(),
        bad_args: "List names.\n".to_string(),
        contract: "Full parameters of {name}:\n".to_string(),
    }
}

fn split(tools: &[ToolEntry]) -> (Vec<ToolSpec>, BTreeMap<String, ToolRule>) {
    crate::tools::split(tools).unwrap()
}

#[test]
fn tools_that_need_loading_show_a_shell_and_load_tools_joins_in_order() {
    let tools = [
        entry("alpha", false),
        entry("mcp__x__y", true),
        entry("zeta", true),
    ];
    let (mut face, mut rules) = split(&tools);
    let loading = stub(&tools, Some(&texts()), &mut face, &mut rules)
        .unwrap()
        .expect("有要先加载的");
    let names: Vec<&str> = face.iter().map(|spec| spec.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "load_tools", "mcp__x__y", "zeta"],
        "照名字排进去"
    );
    assert_eq!(face[2].parameters.get(), STUB);
    assert_eq!(face[2].description, "mcp__x__y does it.", "说明整段留着");
    assert_ne!(face[0].parameters.get(), STUB, "不用加载的全量");
    assert_eq!(face[1].description, "Load tools.");
    assert_eq!(
        rules["mcp__x__y"].parameters.get(),
        r#"{"type":"object","properties":{"q":{"type":"string"}}}"#,
        "规则照完整格式修参数"
    );
    assert_eq!(rules["load_tools"].access, Access::Read);
    assert_eq!(loading.tool, "load_tools");
    let contracts: Vec<&String> = loading.contracts.keys().collect();
    assert_eq!(contracts, ["mcp__x__y", "zeta"]);
}

#[test]
fn without_tools_to_load_or_texts_nothing_moves() {
    let tools = [entry("alpha", false), entry("beta", false)];
    let (mut face, mut rules) = split(&tools);
    let before = (face.clone(), rules.len());
    assert!(
        stub(&tools, Some(&texts()), &mut face, &mut rules)
            .unwrap()
            .is_none()
    );
    assert_eq!((face, rules.len()), before);
    let tools = [entry("alpha", true)];
    let (mut face, mut rules) = split(&tools);
    let before = face.clone();
    assert!(
        stub(&tools, None, &mut face, &mut rules).unwrap().is_none(),
        "以前的快照没有那几句字"
    );
    assert_eq!(face, before);
}

#[test]
fn an_entry_without_need_load_keeps_its_old_bytes() {
    let old = serde_json::to_string(&entry("alpha", false)).unwrap();
    assert!(!old.contains("need_load"), "{old}");
    let new = serde_json::to_string(&entry("alpha", true)).unwrap();
    assert!(new.ends_with(r#","need_load":true}"#), "{new}");
    let back: ToolEntry = serde_json::from_str(&old).unwrap();
    assert!(!back.need_load, "以前的快照读成不用加载");
}
