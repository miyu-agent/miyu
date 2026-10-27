//! 装了的功能（施工 F-3 上）：工具归哪个功能；以前的快照记的包编号照现在的功能读。

use super::{Feature, Features};

fn feature(id: &str, package: &str, tools: &[&str]) -> Feature {
    Feature {
        id: id.to_string(),
        package: package.to_string(),
        tools: tools.iter().map(ToString::to_string).collect(),
        service: None,
        script: None,
    }
}

/// 基础系统两个功能、人格记忆一个没写工具、接入QQ 一个没写工具。
fn installed() -> Features {
    Features::new(vec![
        feature("files", "basesystem", &["read", "write"]),
        feature("commands", "basesystem", &["shell"]),
        feature("memory", "memory", &[]),
        feature("qq", "onebot", &[]),
    ])
}

#[test]
fn a_tool_belongs_to_the_feature_that_lists_it() {
    let features = installed();
    assert_eq!(features.of_tool("basesystem", "write"), Some("files"));
    assert_eq!(features.of_tool("basesystem", "shell"), Some("commands"));
    assert_eq!(
        features.of_tool("basesystem", "jobs"),
        None,
        "几个功能都没列它"
    );
}

#[test]
fn a_package_with_one_feature_owns_all_its_tools() {
    let features = installed();
    assert_eq!(features.of_tool("memory", "remember"), Some("memory"));
    assert_eq!(features.of_tool("onebot", "qq_send"), Some("qq"));
    assert_eq!(features.of_tool("nowhere", "x"), None, "没装的包");
}

#[test]
fn a_listed_tool_wins_even_in_a_package_with_one_feature() {
    let features = Features::new(vec![feature("only", "p", &["a"])]);
    assert_eq!(features.of_tool("p", "a"), Some("only"));
    assert_eq!(features.of_tool("p", "b"), Some("only"));
}

#[test]
fn installed_means_listed() {
    let features = installed();
    assert!(features.installed("qq"));
    assert!(!features.installed("onebot"), "包的编号不是功能");
    assert!(!features.installed("roleplay"));
    let ids: Vec<&str> = features.iter().map(|feature| feature.id.as_str()).collect();
    assert_eq!(ids, ["files", "commands", "memory", "qq"]);
}

#[test]
fn old_package_ids_read_as_their_features() {
    let features = installed();
    let old = ["basesystem", "memory", "onebot", "roleplay"].map(String::from);
    let read: Vec<String> = features.read_legacy(&old).into_iter().collect();
    assert_eq!(
        read,
        ["commands", "files", "memory", "qq", "roleplay"],
        "包换成它的功能；功能编号照旧；不认识的照旧"
    );
}

/// 现在的快照记的是功能的编号：哪怕有个包恰好叫这个名字，也不换成那个包的功能（施工 F-3 上）。
#[test]
fn a_feature_id_is_not_read_as_a_package_of_the_same_name() {
    let features = Features::new(vec![
        feature("files", "basesystem", &["read"]),
        feature("sync", "files", &[]),
    ]);
    let read: Vec<String> = features
        .read_legacy(&["files".to_string()])
        .into_iter()
        .collect();
    assert_eq!(read, ["files"]);
}

/// MCP 服务的工具（施工 X-2）：照名字的前缀 `mcp__<服务名>__` 归那个服务的功能；包自己的工具照旧照包自己的功能算，MCP 服务的
/// 功能不算在「只有一个功能」里。
#[test]
fn an_mcp_tool_belongs_to_its_server_and_the_package_keeps_its_own() {
    let server = |name: &str| Feature {
        id: format!("mcp-{name}"),
        package: "relay".to_string(),
        tools: Vec::new(),
        service: Some(name.to_string()),
        script: None,
    };
    let features = Features::new(vec![
        feature("relay", "relay", &[]),
        server("github"),
        server("git"),
    ]);
    assert_eq!(
        features.of_tool("relay", "mcp__github__create_issue"),
        Some("mcp-github")
    );
    assert_eq!(features.of_tool("relay", "mcp__git__log"), Some("mcp-git"));
    assert_eq!(
        features.of_tool("relay", "relay_send"),
        Some("relay"),
        "包自己只有一个功能，自己的工具都归它"
    );
    assert_eq!(
        features.of_tool("relay", "mcp__gitlab__x"),
        Some("relay"),
        "没有这个服务的照包自己的算"
    );
    assert_eq!(
        features.of_tool("other", "mcp__github__create_issue"),
        None,
        "别的包的不算"
    );
}

/// 脚本的工具（施工 X-12 中）：照名字归那个脚本的功能；包自己的工具照旧照包自己的功能算，脚本的功能不算在「只有一个功能」里。
#[test]
fn a_script_tool_belongs_to_its_script_and_the_package_keeps_its_own() {
    let script = |name: &str| Feature {
        id: format!("script-{name}"),
        package: "tools".to_string(),
        tools: Vec::new(),
        service: None,
        script: Some(name.to_string()),
    };
    let features = Features::new(vec![
        feature("tools", "tools", &[]),
        script("weather"),
        script("bangumi"),
    ]);
    assert_eq!(features.of_tool("tools", "weather"), Some("script-weather"));
    assert_eq!(features.of_tool("tools", "bangumi"), Some("script-bangumi"));
    assert_eq!(
        features.of_tool("tools", "own_tool"),
        Some("tools"),
        "包自己的工具照旧归它唯一的那个功能"
    );
    assert_eq!(features.of_tool("other", "weather"), None, "别的包的不算");
}

/// 扩展交的功能（施工 IM-6）：列了的工具归它，没列的照旧归包只有的那一个功能；算装了，接在最后；整份换掉。
#[test]
fn an_offered_feature_takes_the_tools_it_lists() {
    let features = installed().offering(vec![feature("qq-history", "onebot", &["qq_history"])]);
    assert_eq!(features.of_tool("onebot", "qq_history"), Some("qq-history"));
    assert_eq!(
        features.of_tool("onebot", "qq_send"),
        Some("qq"),
        "没列的照旧"
    );
    assert_eq!(
        features.of_tool("memory", "qq_history"),
        Some("memory"),
        "别的包的不算"
    );
    assert!(features.installed("qq-history"));
    assert_eq!(
        features.iter().last().map(|one| one.id.as_str()),
        Some("qq-history")
    );
    let cleared = features.offering(Vec::new());
    assert!(!cleared.installed("qq-history"));
    assert_eq!(cleared.of_tool("onebot", "qq_history"), Some("qq"));
}
