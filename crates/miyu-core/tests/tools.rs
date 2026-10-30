//! 核心起来时登记基础系统（施工 4-4 上）：工具目录里有读的三件（施工 4-4 下）、写的三件（施工 4-6）、`shell`（施工 4-8）、`history`（施工 6-4）、`subagent`（施工 7-5，7-5 再补改名）、`jobs`（施工 7-4）、`send_message`（施工 7-7，施工 C-5 从 `message_agent` 改名）、`sessions`（施工 C-3）、`session_usage`（施工 8-15）、`ask_user`（施工 D-2）和 `todowrite`（施工 D-3），记忆这个软件包的 `remember`、`forget`、`memory_search`（施工 R-3 中），长期目标这个软件包的 `goal`（施工 X-8 上），联网这个软件包的 `web_fetch`（施工 N-1）、`web_search`（施工 N-2 上）；资源目录坏了，
//! 说是哪一份。

use std::path::Path;

use miyu_core::packages::NET;
use miyu_goal::PACKAGE as GOAL;
use miyu_knowledge::PACKAGE as KNOWLEDGE;
use miyu_memory::PACKAGE as MEMORY;
use miyu_store::packages::{Found, Packages};
use miyu_store::resources::ResourceRoot;

/// 仓库的资源目录。
fn shipped_resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 出厂的那一层清单，去掉 `without` 这几个包（施工 F-2：没装的就是没有清单）。
fn installed(resources: &ResourceRoot, without: &[&str]) -> Vec<Found> {
    let mut found = Packages::shipped(resources).read();
    found.retain(|one| !without.contains(&one.id.as_str()));
    found
}

/// 原来基础系统组里的包：`miyu-basesystem` 的十八个，加上定时、循环两个（施工 J-2，它们的代码在 `miyu-schedule`）。施工 FG-1 起
/// 待办、定时、循环进了任务和定时组，项目文档进了注入环境信息组，这里照旧一起卸。
fn group() -> Vec<&'static str> {
    let mut group = miyu_basesystem::PACKAGES.to_vec();
    group.extend([miyu_schedule::SCHEDULE, miyu_schedule::LOOP]);
    group
}

/// 工具目录里的名字，照名字排。
fn names(found: &[Found]) -> Vec<String> {
    let resources = shipped_resources();
    let catalog = miyu_core::tools(&resources, found).expect("出厂的资源读得出来");
    catalog.specs().map(|spec| spec.name.clone()).collect()
}

#[test]
fn the_catalog_has_the_base_system() {
    let resources = shipped_resources();
    let catalog =
        miyu_core::tools(&resources, &installed(&resources, &[])).expect("出厂的资源读得出来");
    let names: Vec<&str> = catalog.specs().map(|spec| spec.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "ask_user",
            "cron_create",
            "cron_delete",
            "cron_list",
            "edit",
            "forget",
            "glob",
            "goal",
            "grep",
            "history",
            "jobs",
            "kb_delete",
            "kb_edit",
            "kb_read",
            "kb_save",
            "kb_search",
            "memory_search",
            "package",
            "packages",
            "read",
            "remember",
            "schedule_wakeup",
            "send_message",
            "session_usage",
            "sessions",
            "shell",
            "show",
            "subagent",
            "todowrite",
            "trash",
            "web_fetch",
            "web_search",
            "write"
        ]
    );
}

/// 内置包照清单启用（施工 F-2，设计 30 第二节第 3 条）：没有清单的，它的工具不登记。
#[test]
fn a_builtin_without_its_manifest_brings_no_tools() {
    let resources = shipped_resources();
    let names_without = |without: &[&str]| names(&installed(&resources, without));
    let without_memory = names_without(&[MEMORY]);
    assert!(without_memory.contains(&"read".to_string()));
    for tool in ["remember", "forget", "memory_search"] {
        assert!(
            !without_memory.contains(&tool.to_string()),
            "没装记忆，没有 {tool}"
        );
    }
    assert!(
        !names_without(&[GOAL]).contains(&"goal".to_string()),
        "没装长期目标，没有 goal"
    );
    let without_knowledge = names_without(&[KNOWLEDGE]);
    for tool in ["kb_search", "kb_read", "kb_save", "kb_edit", "kb_delete"] {
        assert!(
            !without_knowledge.contains(&tool.to_string()),
            "没装知识库，没有 {tool}"
        );
    }
    assert!(
        !names_without(&[NET])
            .iter()
            .any(|name| name.starts_with("web_")),
        "没装联网，没有 web_fetch、web_search"
    );
    // 施工 BS-1 上中起基础系统是一个组：卸掉组里的全部包，才一件都没有；只卸 shell 的只少它。
    let mut without: Vec<&str> = group();
    without.extend([KNOWLEDGE, NET]);
    assert_eq!(
        names_without(&without),
        ["forget", "goal", "memory_search", "remember"],
        "没装基础系统组、知识库、联网，只有记忆的三件和长期目标的一件"
    );
    let without_shell = names_without(&["miyu-base-shell"]);
    assert!(
        !without_shell.contains(&"shell".to_string()),
        "没装 shell 包，没有 shell"
    );
    assert!(without_shell.contains(&"read".to_string()), "别的包照旧");
    let without_sessions = names_without(&["miyu-base-sessions"]);
    assert!(
        !without_sessions.contains(&"send_message".to_string())
            && !without_sessions.contains(&"sessions".to_string()),
        "sessions 包带着 send_message 一起没"
    );
    let mut only_knowledge: Vec<&str> = group();
    only_knowledge.extend([MEMORY, GOAL, NET]);
    assert_eq!(
        names_without(&only_knowledge),
        ["kb_delete", "kb_edit", "kb_read", "kb_save", "kb_search"],
        "只装知识库（施工 K-1 下、K-3），只有它的五件"
    );
    let mut only_net: Vec<&str> = group();
    only_net.extend([MEMORY, GOAL, KNOWLEDGE]);
    assert_eq!(
        names_without(&only_net),
        ["web_fetch", "web_search"],
        "只装联网（施工 N-1、N-2 上），只有它的两件"
    );
    let mut none: Vec<&str> = group();
    none.extend([MEMORY, GOAL, KNOWLEDGE, NET]);
    assert!(names_without(&none).is_empty());
}

/// 写错了的清单不算装了（施工 F-2）：照读成了的认。
#[test]
fn a_broken_manifest_does_not_count_as_installed() {
    let resources = shipped_resources();
    let mut found = installed(&resources, &[]);
    for one in &mut found {
        if one.id == MEMORY {
            one.read = Err(miyu_store::packages::Issue::Unreadable(
                std::io::Error::other("坏了"),
            ));
        }
    }
    assert!(!names(&found).contains(&"remember".to_string()));
}

/// 出厂的内置包清单，这一份核心都编进来了；编进来的也都有出厂的清单（施工 F-2，默认的 cargo 开关）。
#[test]
fn every_shipped_builtin_is_compiled_in() {
    let resources = shipped_resources();
    let mut shipped: Vec<String> = installed(&resources, &[])
        .into_iter()
        .filter(|one| {
            one.read
                .as_ref()
                .is_ok_and(|manifest| manifest.kind == miyu_config::package::PackageKind::Builtin)
        })
        .map(|one| one.id)
        .collect();
    shipped.sort();
    let mut built_in: Vec<String> = miyu_core::built_in()
        .iter()
        .map(ToString::to_string)
        .collect();
    built_in.sort();
    assert_eq!(shipped, built_in);
}

#[test]
fn broken_resources_say_which_file() {
    let resources = ResourceRoot::at(std::env::temp_dir().join("miyu-core-no-resources-here"));
    let found = installed(&shipped_resources(), &[]);
    let error = miyu_core::tools(&resources, &found).expect_err("读不出来");
    // 先读的是几件工具共用的那几句。
    assert!(error.contains("missing.txt"), "{error}");
}

/// 出厂的每一件工具都归一个功能（施工 F-3 上，设计 30 第三节第 6 条）：预设照功能开关，归不上的就关不掉。
#[test]
fn every_shipped_tool_belongs_to_a_feature() {
    let resources = shipped_resources();
    let found = installed(&resources, &[]);
    let features = miyu_endpoint::packages::features(&found);
    let catalog = miyu_core::tools(&resources, &found).expect("出厂的资源读得出来");
    let mut owners = Vec::new();
    for spec in catalog.specs() {
        let package = catalog.package_of(&spec.name).expect("记了包");
        let feature = features
            .of_tool(package, &spec.name)
            .unwrap_or_else(|| panic!("{} 没有归哪个功能", spec.name));
        owners.push((spec.name.clone(), feature.to_string()));
    }
    let owner = |tool: &str| {
        owners
            .iter()
            .find(|(name, _)| name == tool)
            .map(|(_, feature)| feature.as_str())
    };
    // 施工 BS-1 上中起基础系统一件一个包，包就是它的功能。
    assert_eq!(owner("shell"), Some("shell"));
    assert_eq!(owner("jobs"), Some("jobs"));
    assert_eq!(owner("edit"), Some("edit"));
    assert_eq!(owner("send_message"), Some("sessions"));
    assert_eq!(owner("package"), Some("packages"));
    assert_eq!(owner("remember"), Some("memory"));
}

/// 基础系统「工具 → 包」的对照（施工 PN-3：包改名成 `miyu-base-*` 以后不再是工具名等于包编号）：每件工具归的包，就是出厂清单里
/// `[features] tools` 列着它的那一个；不是基础系统的照组名。联网包的编号核心另记的一份和 `miyu-net` 的一样。
#[test]
fn base_system_tools_go_to_the_package_whose_manifest_lists_them() {
    let found = installed(&shipped_resources(), &[]);
    for (tool, package) in miyu_basesystem::TOOLS {
        let listing: Vec<&str> = found
            .iter()
            .filter(|one| {
                one.read.as_ref().is_ok_and(|manifest| {
                    manifest
                        .features_of(&one.id)
                        .iter()
                        .any(|feature| feature.tools.iter().any(|name| name == tool))
                })
            })
            .map(|one| one.id.as_str())
            .collect();
        assert_eq!(listing, [package], "{tool}");
        assert_eq!(miyu_basesystem::package_of(tool), package, "{tool}");
        assert!(miyu_basesystem::PACKAGES.contains(&package), "{tool}");
    }
    assert_eq!(miyu_basesystem::package_of("nope"), miyu_basesystem::GROUP);
    assert_eq!(NET, miyu_net::PACKAGE);
}

/// 交给端点的内置包工具的端口（施工 F-5 中）：照清单交回的和起来时登记的一样，没装的包不在里面。
#[test]
fn the_builtin_port_gives_what_startup_registers() {
    let resources = shipped_resources();
    let port = miyu_core::builtin_tools(&resources, &miyu_store::env::Env::current());
    let groups = port.tools(&installed(&resources, &[])).expect("拼得出");
    let mut from_port: Vec<String> = groups
        .iter()
        .flat_map(|(_, tools)| tools.iter().map(|tool| tool.spec().name.clone()))
        .collect();
    from_port.sort();
    assert_eq!(from_port, names(&installed(&resources, &[])));
    let without = port
        .tools(&installed(&resources, &[MEMORY]))
        .expect("拼得出");
    assert!(without.iter().all(|(id, _)| id != MEMORY));
}

/// 同一个端口交的整份配置清单（施工 F-5 补）：和起来时读配置用的一样；人格记忆没装的，它那几项照样在、设置页不画。
#[test]
fn the_builtin_port_gives_the_settings_startup_reads() {
    let resources = shipped_resources();
    let port = miyu_core::builtin_tools(&resources, &miyu_store::env::Env::current());
    let all = port.settings(&mut installed(&resources, &[]));
    let startup = miyu_core::settings::Packaged::of(&mut installed(&resources, &[])).all();
    assert_eq!(all, startup);
    let hidden = |items: &[miyu_config::Item]| -> Vec<&str> {
        items
            .iter()
            .filter(|item| item.ui.hidden)
            .map(|item| item.key)
            .collect()
    };
    assert!(
        !hidden(&all).iter().any(|key| key.starts_with("memory.")),
        "装着的画"
    );
    let without = port.settings(&mut installed(&resources, &[MEMORY]));
    // 每个装着的包那一项关包的开关（施工 U23 末上）跟着装没装：不算进「照样认」的那几项。
    let off = |items: &[miyu_config::Item]| -> Vec<&str> {
        items
            .iter()
            .filter(|item| item.key.starts_with("packages.") && item.key.ends_with(".off"))
            .map(|item| item.key)
            .collect()
    };
    assert_eq!(
        without.len() - off(&without).len(),
        all.len() - off(&all).len(),
        "没装的照样认"
    );
    assert!(
        off(&all).contains(&"packages.miyu-base-memory.off"),
        "装着的有它那一项开关"
    );
    assert!(
        !off(&without).contains(&"packages.miyu-base-memory.off"),
        "没装的没有"
    );
    assert!(
        hidden(&without)
            .iter()
            .any(|key| key.starts_with("memory.")),
        "没装的不画"
    );
}

/// 同一个端口照清单拼本机的向量模型（施工 F-5 再补）：人格记忆推荐、内置语义模型那个小程序包也装着才有，包目录照它的。
#[test]
fn the_builtin_port_sets_up_the_local_model_from_the_list() {
    let resources = shipped_resources();
    let port = miyu_core::builtin_tools(&resources, &miyu_store::env::Env::current());
    let shipped = installed(&resources, &[]);
    let read =
        |text: &str| miyu_config::package::read(text).map_err(miyu_store::packages::Issue::Wrong);
    let embed = Found {
        id: "miyu-embed".to_string(),
        layer: miyu_store::packages::Layer::Home,
        path: std::env::temp_dir().join("miyu-embed").join("package.toml"),
        read: read(
            "[package]\nprotocol = [1, 1]\nname = { en = \"Model\" }\n\n[worker]\nprogram = \"miyu-embed\"\n",
        ),
    };
    let mut with: Vec<&Found> = shipped.iter().collect();
    assert!(port.embed(&with).is_none(), "出厂不装");
    with.push(&embed);
    let setup = port.embed(&with).expect("装上就有");
    assert_eq!(setup.dir, embed.files_dir());
    assert_eq!(setup.manifest, embed.files_dir().join("model.toml"));
}

/// 出厂的工具里，脚本里能调的正好这八件（施工 M-2；`history` 是 M-2 补，`sessions`、`session_usage` 是 M-2 再补，
/// 2026-10-21 项目主人定的通则；设计 39 第三节第 2 条；`packages` 施工 BS-1 中起不上工具面，改由 `miyu pkg` 调），别的都不能；
/// 能调的都是读、联网的。
#[test]
fn exactly_eight_shipped_tools_are_callable_from_scripts() {
    let resources = shipped_resources();
    let catalog =
        miyu_core::tools(&resources, &installed(&resources, &[])).expect("出厂的资源读得出来");
    let on: Vec<&str> = catalog
        .specs()
        .filter(|spec| spec.clicall)
        .map(|spec| spec.name.as_str())
        .collect();
    assert_eq!(
        on,
        [
            "history",
            "kb_read",
            "kb_search",
            "memory_search",
            "session_usage",
            "sessions",
            "web_fetch",
            "web_search"
        ]
    );
    for spec in catalog.specs().filter(|spec| spec.clicall) {
        assert!(
            matches!(
                spec.access,
                miyu_kernel::tool::Access::Read | miyu_kernel::tool::Access::Network
            ),
            "{} 是 {:?}",
            spec.name,
            spec.access
        );
    }
}
