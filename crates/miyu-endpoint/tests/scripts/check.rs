//! `miyu check` 报脚本、技能、斜杠模板（施工 X-12 下）：跳过的脚本、技能，撞名没登记上的脚本；链接指到数据根里不认的地方的
//! （施工 X-37）。

use super::*;

#[tokio::test]
async fn check_reports_skipped_and_clashed_scripts_and_skipped_skills() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    home.write("home/alice/scripts/read.py", &echo("read"));
    home.write("home/alice/scripts/broken.py", "#!/bin/sh\necho hi\n");
    home.write(
        "home/alice/skills/notes/SKILL.md",
        "---\nname: notes\n---\nbody\n",
    );
    let mut client = Client::connect(core(&home, &Script::new([])));
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let problems: Vec<(String, String, String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("kind"), text("file"), text("code"), text("level"))
        })
        .collect();
    let row = |kind: &str, file: &str, code: &str| {
        (
            kind.to_string(),
            file.to_string(),
            code.to_string(),
            "warning".to_string(),
        )
    };
    assert_eq!(
        problems,
        [
            row("script", "home/alice/scripts/broken.py", "script_skipped"),
            row("script", "home/alice/scripts/read.py", "script_clashed"),
            row("skill", "home/alice/skills/notes/SKILL.md", "skill_skipped"),
        ],
        "{reply}"
    );
    let messages: Vec<&str> = reply["result"]["problems"]
        .as_array()
        .expect("是一处处")
        .iter()
        .map(|one| one["message"].as_str().unwrap_or_default())
        .collect();
    assert!(messages[0].contains("Description is missing"), "{reply}");
    assert!(messages[1].contains("read"), "{reply}");
    assert!(messages[2].contains("description"), "{reply}");
}

/// 链接（施工 X-37）：指到数据根里不认的地方（`system/`）的脚本、技能、斜杠模板各报 `*_link_outside`，说指到了哪一片；断了的
/// 照跳过报。Unix 上（Windows 上建链接要权限，找法的测试在 `miyu-store` 里三个平台都跑）。
#[cfg(unix)]
#[tokio::test]
async fn check_reports_links_out_of_the_accounts_folders() {
    let home = Home::new();
    home.write(
        "system/keys/SKILL.md",
        "---\nname: keys\ndescription: Not mine.\n---\n",
    );
    home.write("system/token.txt", "secret\n");
    let link = |target: &str, at: &str| {
        let at = home.root.path().join(at);
        std::fs::create_dir_all(at.parent().expect("有上级")).expect("建得了");
        std::os::unix::fs::symlink(home.root.path().join(target), at).expect("建得了链接");
    };
    link("system/token.txt", "home/alice/scripts/token.py");
    link("nowhere.py", "home/alice/scripts/gone.py");
    link("system/keys", "home/alice/skills/keys");
    link("system/token.txt", "home/alice/commands/token.md");
    let mut client = Client::connect(core(&home, &Script::new([])));
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let problems: Vec<(String, String, String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("kind"), text("file"), text("code"), text("message"))
        })
        .collect();
    let codes: Vec<(&str, &str, &str)> = problems
        .iter()
        .map(|(kind, file, code, _)| (kind.as_str(), file.as_str(), code.as_str()))
        .collect();
    assert_eq!(
        codes,
        [
            ("script", "home/alice/scripts/gone.py", "script_skipped"),
            (
                "script",
                "home/alice/scripts/token.py",
                "script_link_outside"
            ),
            ("skill", "home/alice/skills/keys", "skill_link_outside"),
            (
                "slash",
                "home/alice/commands/token.md",
                "slash_link_outside"
            ),
        ],
        "{reply}"
    );
    assert!(problems[0].3.contains("broken link"), "{reply}");
    for (n, part) in [
        (1, "system/token.txt"),
        (2, "system/keys"),
        (3, "system/token.txt"),
    ] {
        assert!(problems[n].3.contains(part), "说指到了哪一片：{reply}");
    }
}
