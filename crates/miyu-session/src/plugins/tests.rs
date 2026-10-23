//! 场所插件（施工 O-60）：带标记的工具只有场所开着才给，没说过的不给；本机会话照给；不带标记的照旧；场所之间互不相干。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Spec, Tool};

use super::{VenuePlugins, present, replug, sift};

/// 带场所插件 `plugin` 的一件假工具。
fn plugged(name: &str, plugin: &str) -> Arc<dyn Tool> {
    let spec = Spec {
        plugin: Some(plugin.to_string()),
        ..Fake::new(name, Access::Read, Act::Echo).spec().clone()
    };
    Fake::from_spec(spec, Act::Echo)
}

/// 基础系统一件 `read`；桥 `onebot` 登记的 `qq_mute`（插件 `qq-admin`）、`qq_send`（不带标记）；包 `sponsor` 登记的
/// `sponsor_list`（插件 `miyu-im-sponsor`）。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([("basesystem", vec![fake("read")])])
        .expect("合写法")
        .replacing(
            "onebot",
            vec![plugged("qq_mute", "qq-admin"), fake("qq_send")],
        )
        .expect("合写法")
        .replacing("sponsor", vec![plugged("sponsor_list", "miyu-im-sponsor")])
        .expect("合写法")
}

fn venue(text: &str) -> VenueId {
    VenueId::parse(text).expect("合写法")
}

fn on(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn plugin_tools_are_given_only_where_turned_on() {
    let catalog = catalog();
    let plugins = VenuePlugins::default();
    let group = venue("qq:group:1");
    assert!(!plugins.gives(&group, &catalog, "qq_mute"), "没说过的不给");
    assert!(plugins.gives(&group, &catalog, "qq_send"), "不带标记的照旧");
    assert!(plugins.gives(&group, &catalog, "read"));
    plugins.set(group.clone(), on(&["qq-admin"]));
    assert!(plugins.gives(&group, &catalog, "qq_mute"));
    assert!(
        !plugins.gives(&group, &catalog, "sponsor_list"),
        "别的插件照旧不给"
    );
    assert!(
        !plugins.gives(&venue("qq:group:2"), &catalog, "qq_mute"),
        "别的场所照旧"
    );
    plugins.set(group.clone(), on(&[]));
    assert!(
        !plugins.gives(&group, &catalog, "qq_mute"),
        "空的就是全不给"
    );
    assert!(plugins.on(&group).is_empty());
}

#[test]
fn local_sessions_ignore_the_mark() {
    let catalog = catalog();
    let plugins = VenuePlugins::default();
    assert!(plugins.gives(&venue("local"), &catalog, "qq_mute"));
    assert!(plugins.gives(&venue("local"), &catalog, "sponsor_list"));
}

#[test]
fn the_face_is_sifted_and_replugged() {
    let catalog = catalog();
    let plugins = VenuePlugins::default();
    let group = venue("qq:group:1");
    let entries = |names: &[&str]| -> Vec<miyu_policy::ToolEntry> {
        names
            .iter()
            .map(|name| miyu_policy::ToolEntry {
                name: (*name).to_string(),
                description: String::new(),
                parameters: serde_json::from_str("{}").expect("合写法"),
                access: Access::Read,
                need_load: false,
                clicall: false,
            })
            .collect()
    };
    let mut face = entries(&["qq_mute", "qq_send", "read", "sponsor_list"]);
    assert!(sift(&mut face, &catalog, (&plugins, &group)).is_empty());
    let names: Vec<&str> = face.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["qq_send", "read"]);
    plugins.set(group.clone(), on(&["miyu-im-sponsor"]));
    let fresh = entries(&["qq_mute", "qq_send", "read", "sponsor_list"]);
    let face = replug(face, &fresh, &catalog, (&plugins, &group));
    assert_eq!(present(&face, &catalog), ["sponsor_list"], "开了的补上");
    assert!(
        face.iter().any(|entry| entry.name == "qq_send"),
        "不带标记的留着"
    );
}
