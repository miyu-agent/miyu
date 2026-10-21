//! system 尾部那几段怎么挑（施工 X-20、X-39）：工具在面上的包带它的；MCP 工具照服务带说明那一段；MCP 的只在工具面变了的那一轮
//! 照这一刻的换，没变的照钉下的；扩展放的照说的、没说过的照钉下的。

use std::sync::Arc;

use miyu_kernel::tool::Access;
use miyu_tool::Tool;
use miyu_tool::testkit::{Act, Fake};

use super::*;

/// 工具面：几个名字。
fn face(names: &[&str]) -> Vec<ToolEntry> {
    names
        .iter()
        .map(|name| ToolEntry {
            name: (*name).to_string(),
            description: String::new(),
            parameters: serde_json::from_str("{}").expect("是 JSON"),
            access: Access::Read,
            need_load: false,
            clicall: false,
        })
        .collect()
}

/// 目录：包 `memes` 的 `meme_send`，包 `kit` 带的服务 `github` 的一件。
fn catalog() -> Catalog {
    let fake = |name: &str| Fake::new(name, Access::Read, Act::Echo) as Arc<dyn Tool>;
    Catalog::in_packages([
        ("memes", vec![fake("meme_send")]),
        ("kit", vec![fake("mcp__github__issues")]),
    ])
    .expect("合写法")
}

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, text)| ((*key).to_string(), (*text).to_string()))
        .collect()
}

fn asked(pairs: &[(&str, Option<&str>)]) -> BTreeMap<String, Option<String>> {
    pairs
        .iter()
        .map(|(key, text)| ((*key).to_string(), text.map(str::to_string)))
        .collect()
}

#[test]
fn an_mcp_tool_brings_its_service_instructions() {
    let catalog = catalog();
    let asked = asked(&[
        ("mcp:github", Some("Use issues.")),
        ("memes", Some("Send memes.")),
    ]);
    let got = resolve(
        &catalog,
        &face(&["mcp__github__issues"]),
        (&BTreeMap::new(), &asked),
        true,
    );
    assert_eq!(
        got,
        map(&[("mcp:github", "Use issues.")]),
        "包 kit 没放过，memes 的工具不在面上"
    );
    assert_eq!(mcp_key("github"), "mcp:github");
}

#[test]
fn instructions_change_only_with_the_tool_face() {
    let catalog = catalog();
    let pinned = map(&[("mcp:github", "Old.")]);
    let asked = asked(&[("mcp:github", Some("New."))]);
    let tools = face(&["mcp__github__issues"]);
    assert_eq!(
        resolve(&catalog, &tools, (&pinned, &asked), false),
        pinned,
        "工具面没变：照钉下的"
    );
    assert_eq!(
        resolve(&catalog, &tools, (&pinned, &asked), true),
        map(&[("mcp:github", "New.")]),
        "工具面变了：照这一刻的"
    );
    assert_eq!(
        resolve(&catalog, &tools, (&pinned, &BTreeMap::new()), true),
        BTreeMap::new(),
        "工具面变了、这时没交说明：没有"
    );
}

#[test]
fn extension_text_still_follows_what_it_said() {
    let catalog = catalog();
    let pinned = map(&[("memes", "Old.")]);
    let tools = face(&["meme_send"]);
    let said = asked(&[("memes", Some("New."))]);
    assert_eq!(
        resolve(&catalog, &tools, (&pinned, &said), false),
        map(&[("memes", "New.")])
    );
    let removed = asked(&[("memes", None)]);
    assert_eq!(
        resolve(&catalog, &tools, (&pinned, &removed), false),
        BTreeMap::new()
    );
    assert_eq!(
        resolve(&catalog, &tools, (&pinned, &BTreeMap::new()), false),
        pinned
    );
}
