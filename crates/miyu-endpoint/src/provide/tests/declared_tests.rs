//! 规格里的两样声明（施工 XG-1 上，设计 `63-扩展工具的效果和声明.md` 第二节）：`paths` 的参数要在 `input_schema` 顶层、是字或字的
//! 数组、要写的访问类别是写；`background_parameters` 的参数要在顶层、不在 `required` 里；写错的 `bad_tool`。工具照 `paths` 报
//! 要碰的路径；不写两格的规格、登记缓存逐字节和以前一样。

use miyu_tool::Target;

use super::*;

/// 一件改文件的工具：`file_path` 是字，`files` 是字的数组，`count` 是数，`edits` 是对象的数组；`file_path` 必填。
fn editor(access: &str) -> ToolParams {
    serde_json::from_value(json!({
        "name": "ext_edit", "description": "Edit a file.",
        "input_schema": {"type": "object", "properties": {
            "file_path": {"type": "string"},
            "files": {"type": "array", "items": {"type": "string"}},
            "count": {"type": "integer"},
            "edits": {"type": "array", "items": {"type": "object"}},
            "in_background": {"type": "boolean"},
        }, "required": ["file_path"]},
        "access": access, "venues": ["local"]
    }))
    .expect("合写法")
}

/// 写了 `paths` 的一件。
fn with_paths(access: &str, paths: Value) -> ToolParams {
    let mut tool = editor(access);
    tool.paths = Some(paths);
    tool
}

/// 写了 `background_parameters` 的一件。
fn with_background(names: Value) -> ToolParams {
    let mut tool = editor("write");
    tool.background_parameters = Some(names);
    tool
}

#[test]
fn paths_name_text_parameters_and_writes_need_write_access() {
    let declared = checked(with_paths(
        "write",
        json!([{"parameter": "file_path", "write": true}, {"parameter": "files"}]),
    ))
    .expect("合写法");
    assert_eq!(
        declared.paths,
        [
            declared::PathParameter {
                parameter: "file_path".to_string(),
                write: true
            },
            declared::PathParameter {
                parameter: "files".to_string(),
                write: false
            },
        ]
    );
    // 只读的工具声明读的路径：照收。
    assert!(checked(with_paths("read", json!([{"parameter": "file_path"}]))).is_ok());
    for (access, paths) in [
        // 要写的，访问类别得是写。
        ("read", json!([{"parameter": "file_path", "write": true}])),
        (
            "execute",
            json!([{"parameter": "file_path", "write": true}]),
        ),
        // 参数格式里没有的、不是字的、对象的数组、重复的。
        ("write", json!([{"parameter": "path"}])),
        ("write", json!([{"parameter": "count"}])),
        ("write", json!([{"parameter": "edits"}])),
        (
            "write",
            json!([{"parameter": "file_path"}, {"parameter": "file_path", "write": true}]),
        ),
        // 写法不对：不是数组、项不是对象、没有参数名、多写了格、`write` 不是布尔。
        ("write", json!({"parameter": "file_path"})),
        ("write", json!(["file_path"])),
        ("write", json!([{"write": true}])),
        ("write", json!([{"parameter": "file_path", "itself": true}])),
        ("write", json!([{"parameter": "file_path", "write": "yes"}])),
    ] {
        assert_eq!(
            refused_for(with_paths(access, paths.clone())),
            json!({"tool": "ext_edit", "problem": "paths"}),
            "{access} {paths}"
        );
    }
}

#[test]
fn background_parameters_are_optional_parameters_of_the_tool() {
    let declared = checked(with_background(json!(["in_background", "count"]))).expect("合写法");
    assert_eq!(declared.background, ["in_background", "count"]);
    for names in [
        // 必填的去掉了请求就对不上格式；没有的、重复的、不是字的数组。
        json!(["file_path"]),
        json!(["watch"]),
        json!(["count", "count"]),
        json!("in_background"),
        json!([true]),
    ] {
        assert_eq!(
            refused_for(with_background(names.clone())),
            json!({"tool": "ext_edit", "problem": "background_parameters"}),
            "{names}"
        );
    }
}

/// 照 `paths` 报路径：字的一条，字的数组每项一条，照声明的先后；参数缺的、不是字的不出。
#[tokio::test]
async fn the_tool_reports_the_declared_paths() {
    let tool = |paths: Value| {
        RemoteTool::new(
            checked(with_paths("write", paths)).expect("合写法"),
            "editor",
            Arc::new(Provided::default()),
            texts(),
            stores().0,
        )
    };
    let declared = tool(json!([{"parameter": "file_path", "write": true}, {"parameter": "files"}]));
    let call = miyu_tool::testkit::call(r#"{"file_path":"a.txt","files":["b.txt",3,"~/c.txt"]}"#);
    let target = |path: &str, write: bool| Target {
        path: path.to_string(),
        write,
        itself: false,
    };
    assert_eq!(
        declared.targets(&call),
        [
            target("a.txt", true),
            target("b.txt", false),
            target("~/c.txt", false),
        ]
    );
    for args in [r#"{"file_path":7}"#, "{}", "not json"] {
        assert!(
            declared.targets(&miyu_tool::testkit::call(args)).is_empty(),
            "{args}"
        );
    }
    // 没声明的不报路径：照以前「不报路径的调用」判。
    let plain = RemoteTool::new(
        checked(editor("write")).expect("合写法"),
        "editor",
        Arc::new(Provided::default()),
        texts(),
        stores().0,
    );
    assert!(plain.targets(&call).is_empty());
    assert!(plain.background_parameters().is_empty());
    let background = RemoteTool::new(
        checked(with_background(json!(["in_background"]))).expect("合写法"),
        "editor",
        Arc::new(Provided::default()),
        texts(),
        stores().0,
    );
    assert_eq!(background.background_parameters(), ["in_background"]);
}

/// 不写这几格的规格：登记缓存逐字节和以前一样（都不写出来）；写了的原样存、读回来一样（施工 XG-1 下加了 `runs_commands`）。
#[test]
fn specs_without_the_declarations_are_cached_as_before() {
    let before = r#"{"tools":[{"name":"ext_edit","description":"Edit a file.","input_schema":{"type":"object"},"access":"write","venues":["local"]}]}"#;
    let params: ProvideParams = serde_json::from_str(before).expect("合写法");
    assert_eq!(serde_json::to_string(&params).expect("写得成"), before);
    let declared = r#"{"tools":[{"name":"ext_edit","description":"Edit a file.","input_schema":{"type":"object"},"access":"write","venues":["local"],"paths":[{"parameter":"file_path","write":true}],"background_parameters":["in_background"],"runs_commands":true}]}"#;
    let params: ProvideParams = serde_json::from_str(declared).expect("合写法");
    assert_eq!(serde_json::to_string(&params).expect("写得成"), declared);
}

/// `runs_commands`（施工 XG-1 下，设计 63 第二节）：布尔，不写是假；别的写法 `bad_tool`，`problem` 是 `runs_commands`。只有写了真的
/// 才跑命令、接说明末尾那一段（要的字交给它以后）；没交的不接。
#[test]
fn runs_commands_is_a_boolean_and_only_true_runs_commands() {
    let with = |runs: Option<Value>| {
        let mut tool = editor("execute");
        tool.runs_commands = runs;
        tool
    };
    assert!(!checked(with(None)).expect("合写法").runs_commands);
    assert!(
        !checked(with(Some(json!(false))))
            .expect("合写法")
            .runs_commands
    );
    assert!(
        checked(with(Some(json!(true))))
            .expect("合写法")
            .runs_commands
    );
    for bad in [json!("yes"), json!(1), json!([true])] {
        assert_eq!(
            refused_for(with(Some(bad.clone()))),
            json!({"tool": "ext_edit", "problem": "runs_commands"}),
            "{bad}"
        );
    }
    let tool = |runs: Option<Value>| {
        RemoteTool::new(
            checked(with(runs)).expect("合写法"),
            "editor",
            Arc::new(Provided::default()),
            texts(),
            stores().0,
        )
    };
    let runs = tool(Some(json!(true)));
    assert!(runs.runs_commands());
    assert!(!runs.serves_person(), "人跑命令只认内置的");
    assert!(runs.clicall_section("probe", "").is_none(), "没交字的不接");
    let plain = tool(None);
    assert!(!plain.runs_commands());
    assert!(plain.clicall_section("probe", "").is_none());
}
