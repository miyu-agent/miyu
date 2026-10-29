//! `skill.remove`（施工 W-13，网页的技能页）：管理员自己放的整个文件夹挪进回收站，几个一起删；随包来的、没有的拒绝，带名字；
//! 有一个不行就一个都不删；盖住了包里同名的，删了包里的露出来。链接成的只挪链接（施工 X-37）。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::{Client, Home, reason};

/// 一份 `SKILL.md`。
fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Do {name}.\n---\n\nBody.\n")
}

/// 家目录里的 `notes`、盖住出厂同名的 `package-creator`，管理员装的包 `kit` 带的 `kit-notes`；连上（系统的家目录当是数据根）。
async fn connected(home: &Home) -> Client {
    home.write("skills/notes/SKILL.md", &skill("notes"));
    home.write("skills/package-creator/SKILL.md", &skill("package-creator"));
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"kit\" }\n",
    );
    home.write(
        "packages/kit/skills/kit-notes/SKILL.md",
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
        home.root.path().join("skills/notes/SKILL.md").is_file(),
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
    assert!(!home.root.path().join("skills/notes").exists());
    assert_eq!(from(&mut client, "notes").await, None);
    assert_eq!(
        from(&mut client, "package-creator").await,
        Some(json!("shipped")),
        "出厂包里同名的露出来"
    );
}

/// 技能文件夹是链接（施工 X-37，原来找技能不跟链接）：指到数据根外面的照它指到的认，删的时候只挪链接本身、指到的文件夹不动；
/// 指到数据根里别处的不认，也删不到（Unix；Windows 上建链接要权限，找法的测试在 `miyu-store` 里三个平台都跑）。删的那一头
/// 另查链接、文件夹得正好在管理员的技能文件夹下面一层，是兜底。
#[cfg(unix)]
#[tokio::test]
async fn a_linked_folder_is_followed_but_only_the_link_is_removed() {
    let home = Home::new();
    let outside = home.work.join("linked");
    std::fs::create_dir_all(&outside).expect("建得了");
    std::fs::write(outside.join("SKILL.md"), skill("linked")).expect("写得进");
    let inside = home.root.path().join("elsewhere/hidden");
    std::fs::create_dir_all(&inside).expect("建得了");
    std::fs::write(inside.join("SKILL.md"), skill("hidden")).expect("写得进");
    let skills = home.root.path().join("skills");
    std::fs::create_dir_all(&skills).expect("建得了");
    std::os::unix::fs::symlink(&outside, skills.join("linked")).expect("建得了链接");
    std::os::unix::fs::symlink(&inside, skills.join("hidden")).expect("建得了链接");
    let mut client = connected(&home).await;
    assert_eq!(from(&mut client, "linked").await, Some(json!("home")));
    assert_eq!(
        from(&mut client, "hidden").await,
        None,
        "指到数据根里别处的不认"
    );
    let reply = client
        .call("r", "skill.remove", json!({"names": ["hidden"]}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_skill"), "{reply}");
    let reply = client
        .call("r", "skill.remove", json!({"names": ["linked"]}))
        .await;
    if reason(&reply) == Some("trash_failed") {
        return;
    }
    assert!(reply["result"]["trash"].is_array(), "{reply}");
    assert!(
        std::fs::symlink_metadata(skills.join("linked")).is_err(),
        "链接挪走了"
    );
    assert!(outside.join("SKILL.md").is_file(), "指到的文件夹没动");
    assert!(inside.join("SKILL.md").is_file(), "数据根里别处的没动");
    assert_eq!(from(&mut client, "linked").await, None);
}

/// 别的目录里的不删（施工 PT-2 中、PT-2 外）：Codex 这些别的客户端也在用，拒 `skill_shared`、带名字，文件还在。
#[tokio::test]
async fn a_skill_in_another_folder_is_not_removed() {
    let home = Home::new();
    home.write("config.toml", "[skills.external]\nenabled = true\n");
    home.write_elsewhere(".agents/skills", "pdf", "Shared.");
    let shared = home.me().join(".agents/skills/pdf/SKILL.md");
    let mut client = Client::connect(home.core_elsewhere(&Script::new([]), Default::default()));
    client.hello().await;
    assert_eq!(from(&mut client, "pdf").await, Some(json!("agents")));
    let reply = client
        .call("r", "skill.remove", json!({"names": ["pdf"]}))
        .await;
    assert_eq!(reason(&reply), Some("skill_shared"), "{reply}");
    assert_eq!(reply["error"]["data"]["name"], "pdf", "{reply}");
    assert!(shared.is_file());
}
