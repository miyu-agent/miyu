//! 装的时候批能力的算法（施工 PK-3 上）：写法不对的拒；来路没变才接着用；(接着用 ∪ 批的) ∩ 声明的；看一眼的 `held`；她的工具
//! 照看一眼的那一份拼表。

use serde_json::json;

use super::*;

fn came(origin: &str, feed: Option<&str>) -> Came {
    Came {
        origin: origin.to_string(),
        feed: feed.map(str::to_string),
        kind: miyu_store::packages::local::Kind::Official,
    }
}

fn desc(origin: Option<&str>, feed: Option<&str>) -> Desc {
    Desc {
        id: "kit".to_string(),
        version: None,
        installed: "2026-11-05T00:00:00.000Z".to_string(),
        source: String::new(),
        size: 0,
        origin: origin.map(str::to_string),
        feed: feed.map(str::to_string),
        kind: None,
    }
}

fn names(all: &[&str]) -> Vec<String> {
    all.iter().map(|one| (*one).to_string()).collect()
}

#[test]
fn only_known_capabilities_and_ids_are_accepted() {
    let table = |id: &str, name: &str| Approve::from([(id.to_string(), names(&[name]))]);
    assert!(valid(None).is_ok());
    assert!(valid(Some(&table("kit", "network"))).is_ok());
    assert!(
        valid(Some(&table("kit", "login.page"))).is_ok(),
        "装的时候也能批（施工 LP-1）"
    );
    assert!(valid(Some(&table("kit", "teleport"))).is_err(), "不是能力");
    assert!(
        valid(Some(&table("Kit Pack", "network"))).is_err(),
        "编号写法不对"
    );
}

#[test]
fn the_way_in_must_match_to_keep_what_was_approved() {
    let official = came("github.com/a/kit", None);
    let theirs = came("github.com/a/kit", Some("friends"));
    let was = desc(Some("github.com/a/kit"), None);
    assert!(same_way(Some(&was), Some(&official)));
    assert!(!same_way(Some(&was), Some(&theirs)), "换了源");
    assert!(
        !same_way(Some(&was), Some(&came("github.com/b/kit", None))),
        "换了来源仓库"
    );
    assert!(!same_way(Some(&was), None), "本机文件装的");
    assert!(!same_way(None, Some(&official)), "本地库没记的");
    assert!(
        !same_way(Some(&desc(None, None)), Some(&official)),
        "以前是本机文件装的"
    );
}

#[test]
fn approval_is_what_was_kept_or_given_and_still_declared() {
    let declared = [Capability::Network, Capability::Tools];
    assert_eq!(
        settled(
            &names(&["network", "search"]),
            &names(&["tools"]),
            &declared
        ),
        ["network", "tools"],
        "不再声明的去掉，批的加上，照名字排"
    );
    assert_eq!(
        settled(&[], &names(&["tools", "tools"]), &declared),
        ["tools"]
    );
    assert!(settled(&names(&["network"]), &[], &[]).is_empty());
}

#[test]
fn held_marks_only_what_was_approved() {
    let mut listed = json!([{"id": "network"}, {"id": "tools"}]);
    mark_held(&mut listed, &names(&["tools", "search"]));
    assert_eq!(
        listed,
        json!([{"id": "network"}, {"id": "tools", "held": true}])
    );
}

#[test]
fn her_tool_approves_what_the_card_listed() {
    let looked = json!({
        "package": "app", "capabilities": [{"id": "network"}, {"id": "tools", "held": true}],
        "with": [{"package": "lib", "capabilities": [{"id": "search"}]}, {"package": "plain"}],
        "recommends": [{"package": "extra", "capabilities": [{"id": "network"}]}],
    });
    assert_eq!(
        of_looked(&looked),
        Approve::from([
            ("app".to_string(), names(&["network", "tools"])),
            ("lib".to_string(), names(&["search"])),
        ]),
        "推荐的这一次不装，不批"
    );
    let upgrading = json!({"upgrades": [{"package": "app", "capabilities": [{"id": "network"}]}],
                           "with": []});
    assert_eq!(
        of_looked(&upgrading),
        Approve::from([("app".to_string(), names(&["network"]))])
    );
}
