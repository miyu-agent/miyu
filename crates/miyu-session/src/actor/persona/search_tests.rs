//! 供应商自己的联网工具换掉 Miyu 的搜索工具（设计 72 第二节第 2、3 条，施工 DSR-3）：回合开始照模型端口说的判断，激活的
//! 工具面没有 `web_search`、`web_fetch`、快照钉着要声明的；会话中途只改「联网服务」的不变；换了模型、模型这边给不了了
//! 重新判断，换掉的补回来。

use std::sync::Arc;

use miyu_config::{Value, Values};
use miyu_kernel::tool::Access;
use miyu_policy::SearchOffer;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use super::search::SearchNow;
use super::test_support::{names, setup_in, write_preset};
use super::*;

const FLASH: &str = "deepseek/deepseek-flash";

/// 基础系统两件，加上联网的两件。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([(
        "basesystem",
        vec![
            fake("read"),
            fake("shell"),
            fake("web_fetch"),
            fake("web_search"),
        ],
    )])
    .expect("合写法")
}

/// 模型端口这一刻说的：引用是 `model`，`offered` 是模型这边给不给得了。
fn now(model: &str, offered: bool) -> SearchNow {
    SearchNow {
        model: model.to_string(),
        offer: offered.then(|| SearchOffer {
            setting: "deepseek-responses".to_string(),
            replaces: vec![
                ("web_search".to_string(), "deepseek_web_search".to_string()),
                ("web_fetch".to_string(), "deepseek_read_url".to_string()),
            ],
        }),
    }
}

/// 这一轮的配置：「联网服务」选的是 `source`。
fn picked(source: &str) -> Values {
    let mut values = Values::default();
    values.set(
        miyu_config::package::SEARCH_SOURCE,
        Value::Text(source.to_string().into()),
    );
    values
}

fn looked(refresh: &Refresh, values: &Values, search: &SearchNow) -> Seen {
    look(
        refresh,
        values,
        &refresh.tools.edition(),
        Default::default(),
        Some(search),
    )
}

/// 看一遍，要换的换上，交回换没换。
fn step(refresh: &mut Refresh, values: &Values, search: &SearchNow) -> bool {
    match looked(refresh, values, search) {
        Seen::Swapped(snapshot, ..) => {
            refresh.snapshot = snapshot;
            true
        }
        Seen::Same => false,
        _ => panic!("不该读不了"),
    }
}

fn declared(snapshot: &Snapshot) -> Vec<String> {
    super::search::names(snapshot)
}

#[test]
fn an_offering_model_with_the_setting_picked_swaps_the_search_tools() {
    let (_scratch, _root, mut refresh) = setup_in("search-on", "", catalog());
    assert_eq!(
        names(&refresh.snapshot),
        ["read", "shell", "web_fetch", "web_search"]
    );
    // 第一轮开始：模型这边给得了，「联网服务」选了它。
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
    assert_eq!(
        declared(&refresh.snapshot),
        ["deepseek_web_search", "deepseek_read_url"]
    );
    // 下一轮什么都没变：不换，工具面一个字节不动。
    assert!(!step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    // 会话中途把「联网服务」改回自动：这个会话不变（设计 72 第二节第 3 条）。
    assert!(!step(&mut refresh, &picked("auto"), &now(FLASH, true)));
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
}

#[test]
fn a_setting_picked_mid_session_waits_for_a_model_switch() {
    let (_scratch, _root, mut refresh) = setup_in("search-late", "", catalog());
    // 第一轮：给得了，可没选。钉下「没选」，工具面不动。
    assert!(step(&mut refresh, &picked("auto"), &now(FLASH, true)));
    assert_eq!(
        names(&refresh.snapshot),
        ["read", "shell", "web_fetch", "web_search"]
    );
    assert!(declared(&refresh.snapshot).is_empty());
    // 中途选上：这个会话不变。
    assert!(!step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    // 换了模型（还是给得了的一家）：当场重新判断，换上。
    let pro = "deepseek/deepseek-v4-pro";
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(pro, true)
    ));
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
    assert_eq!(declared(&refresh.snapshot).len(), 2);
}

#[test]
fn a_model_that_stops_offering_gets_the_search_tools_back() {
    let (_scratch, _root, mut refresh) = setup_in("search-off", "", catalog());
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
    // 同一个引用，模型这边给不了了（思考设成了关、接口改回了 OpenAI 兼容）：不钉了，两件补回来。
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, false)
    ));
    assert_eq!(
        names(&refresh.snapshot),
        ["read", "shell", "web_fetch", "web_search"]
    );
    assert!(refresh.snapshot.provider_search.is_none());
    assert!(declared(&refresh.snapshot).is_empty());
    // 换到别家模型也一样；给不了的会话快照里没有这一格，以后每一轮都不换。
    assert!(!step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now("openai/gpt", false)
    ));
}

/// 预设关掉了其中一件的：只换工具面里原本有的那一件。
#[test]
fn only_the_tools_the_preset_keeps_are_swapped() {
    let (_scratch, root, mut refresh) =
        setup_in("search-part", "[tools]\nweb_fetch = false\n", catalog());
    assert_eq!(names(&refresh.snapshot), ["read", "shell", "web_search"]);
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
    assert_eq!(declared(&refresh.snapshot), ["deepseek_web_search"]);
    // 预设把抓网页打开：下一轮换快照，新打开的那一件也换成供应商的。
    write_preset(&root, "");
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
    assert_eq!(
        declared(&refresh.snapshot),
        ["deepseek_web_search", "deepseek_read_url"]
    );
}

/// 不问端口的那几处（只干算的预览）照旧快照里钉的。
#[test]
fn a_look_without_the_port_keeps_what_is_pinned() {
    let (_scratch, _root, mut refresh) = setup_in("search-kept", "", catalog());
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    let seen = look(
        &refresh,
        &picked("auto"),
        &refresh.tools.edition(),
        Default::default(),
        None,
    );
    assert!(matches!(seen, Seen::Same));
}

/// 换成的那一件有说明的（`core/hosted/deepseek_web_search.txt`），说明跟着钉的结果进出 system（施工 DSR-4补）：没激活的
/// 会话 system 一个字节不变，会话中途只改「联网服务」的也不变。
#[test]
fn the_hosted_note_is_in_the_system_only_while_swapped() {
    const NOTE: &str = "Web search is `deepseek_web_search`: it runs on the provider side and is not in the function list. `web_fetch` reads one known URL.";
    let (_scratch, _root, mut refresh) = setup_in("search-note", "", catalog());
    let plain = refresh.snapshot.system.clone();
    assert!(!plain.contains("deepseek_web_search"));
    // 给得了、没选：钉下「没选」，system 一个字节不变。
    assert!(step(&mut refresh, &picked("auto"), &now(FLASH, true)));
    assert_eq!(refresh.snapshot.system, plain);
    // 中途选上：这个会话不变。
    assert!(!step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(FLASH, true)
    ));
    assert_eq!(refresh.snapshot.system, plain);
    // 换了模型：重新判断，换上，说明进 system，只一次；读链接那一件没有说明，不带。
    let pro = "deepseek/deepseek-v4-pro";
    assert!(step(
        &mut refresh,
        &picked("deepseek-responses"),
        &now(pro, true)
    ));
    let swapped = refresh.snapshot.system.clone();
    assert_eq!(swapped.matches(NOTE).count(), 1, "{swapped}");
    assert!(!swapped.contains("deepseek_read_url"));
    assert!(swapped.contains(&format!("\n\n{NOTE}")), "单成一段");
    // 下一轮、中途改回自动：一个字节不变。
    assert!(!step(&mut refresh, &picked("auto"), &now(pro, true)));
    assert_eq!(refresh.snapshot.system, swapped);
    // 模型这边给不了了：说明跟着拿掉，system 回到原来的字节。
    assert!(step(&mut refresh, &picked("auto"), &now(pro, false)));
    assert_eq!(refresh.snapshot.system, plain);
}
