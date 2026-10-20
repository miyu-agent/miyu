//! `skill.remove`（施工 W-13，网页的技能页）：管理员自己放的整个文件夹挪进回收站，几个一起删；随包来的、没有的拒绝，带名字；
//! 有一个不行就一个都不删；盖住了包里同名的，删了包里的露出来。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::{Client, Home, reason};

/// 一份 `SKILL.md`。
fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Do {name}.\n---\n\nBody.\n")
}

/// 家目录里的 `notes`、盖住出厂同名的 `package-creator`，管理员装的包 `kit` 带的 `kit-notes`；连上（系统的家目录当是数据根）。
async fn connected(home: &Home) -> Client {
    home.write("home/alice/skills/notes/SKILL.md", &skill("notes"));
    home.write(
        "home/alice/skills/package-creator/SKILL.md",
        &skill("package-creator"),
    );
    home.write(
        "home/alice/packages/kit/package.toml",
        "[package]\nname = { en = \"kit\" }\n",
    );
    home.write(
        "home/alice/packages/kit/skills/kit-notes/SKILL.md",
        &skill("kit-notes"),
    );
    let core = home.core_configured(&Script::new([]), Some(home.root.path().to_path_buf()), &[]);
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

/// 列出来的 `name` 那一项的来处。
async fn from(client: &mut Client, name: &str) -> Option<Value> {
    let reply = client.call("l", "skill.list", json!({})).await;
    reply["result"]["skills"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["name"] == name)
        .map(|one| one["from"].clone())
}

#[tokio::test]
async fn what_cannot_be_removed_is_refused_and_nothing_is_removed() {
    let home = Home::new();
    let mut client = connected(&home).await;
    for (names, want, name) in [
        (json!(["nope"]), "unknown_skill", Some("nope")),
        (
            json!(["notes", "kit-notes"]),
            "skill_from_package",
            Some("kit-notes"),
        ),
        (json!([]), "bad_params", None),
        (json!(["notes", "notes"]), "bad_params", None),
    ] {
        let reply = client
            .call("r", "skill.remove", json!({ "names": names }))
            .await;
        assert_eq!(reason(&reply), Some(want), "{reply}");
        if let Some(name) = name {
            assert_eq!(reply["error"]["data"]["name"], json!(name), "{reply}");
        }
    }
    assert!(
        home.root
            .path()
            .join("home/alice/skills/notes/SKILL.md")
            .is_file(),
        "有一个不行就一个都不删"
    );
}

#[tokio::test]
async fn own_skills_go_to_the_trash_and_what_they_covered_shows_again() {
    let home = Home::new();
    let mut client = connected(&home).await;
    assert_eq!(
        from(&mut client, "package-creator").await,
        Some(json!("home"))
    );
    let reply = client
        .call(
            "r",
            "skill.remove",
            json!({"names": ["notes", "package-creator"]}),
        )
        .await;
    // 有的机器上临时目录那块盘没有能放的回收站：照实报（同 `knowledge_manage.rs`）。
    if reason(&reply) == Some("trash_failed") {
        return;
    }
    let trash = reply["result"]["trash"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(trash.len(), 2, "{reply}");
    assert!(!home.root.path().join("home/alice/skills/notes").exists());
    assert_eq!(from(&mut client, "notes").await, None);
    assert_eq!(
        from(&mut client, "package-creator").await,
        Some(json!("shipped")),
        "出厂包里同名的露出来"
    );
}

/// 技能文件夹是指到别处的链接：找技能不跟链接，删也删不到别处（Unix；Windows 上建链接要权限）。删的那一头另查文件夹得正好在
/// 管理员的技能文件夹下面一层，是兜底：找法以后变了也不会删到别处。
#[cfg(unix)]
#[tokio::test]
async fn a_linked_folder_is_not_followed() {
    let home = Home::new();
    let outside = home.root.path().join("elsewhere/linked");
    std::fs::create_dir_all(&outside).expect("建得了");
    std::fs::write(outside.join("SKILL.md"), skill("linked")).expect("写得进");
    std::fs::create_dir_all(home.root.path().join("home/alice/skills")).expect("建得了");
    std::os::unix::fs::symlink(&outside, home.root.path().join("home/alice/skills/linked"))
        .expect("建得了链接");
    let mut client = connected(&home).await;
    assert_eq!(
        from(&mut client, "linked").await,
        None,
        "找技能本来就不跟链接"
    );
    let reply = client
        .call("r", "skill.remove", json!({"names": ["linked"]}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_skill"), "{reply}");
    assert!(outside.join("SKILL.md").is_file(), "别处的没动");
}
