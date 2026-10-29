//! `miyu check` 报脚本、技能、斜杠模板（施工 X-12 下）：跳过的脚本、技能，撞名没登记上的脚本；链接指到数据根里不认的地方的
//! （施工 X-37）；有状态目录却没有这个脚本的（施工 X-44）；没登记上的分清撞名、造不成工具、核心还没读到（施工 X-46）。

use super::*;

#[tokio::test]
async fn check_reports_skipped_and_clashed_scripts_and_skipped_skills() {
    let home = Home::new();
    home.write("scripts/weather.py", &echo("weather"));
    home.write("scripts/read.py", &echo("read"));
    home.write("scripts/broken.py", "#!/bin/sh\necho hi\n");
    home.write("skills/notes/SKILL.md", "---\nname: notes\n---\nbody\n");
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
            row("script", "scripts/broken.py", "script_skipped"),
            row("script", "scripts/read.py", "script_clashed"),
            row("skill", "skills/notes/SKILL.md", "skill_skipped"),
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

/// 链接（施工 X-37）：指到数据根里不认的地方的脚本、技能、斜杠模板各报 `*_link_outside`，说指到了哪一片；断了的
/// 照跳过报。Unix 上（Windows 上建链接要权限，找法的测试在 `miyu-store` 里三个平台都跑）。
#[cfg(unix)]
#[tokio::test]
async fn check_reports_links_out_of_the_data_root() {
    let home = Home::new();
    home.write(
        "keys/SKILL.md",
        "---\nname: keys\ndescription: Not mine.\n---\n",
    );
    home.write("token.txt", "secret\n");
    let link = |target: &str, at: &str| {
        let at = home.root.path().join(at);
        std::fs::create_dir_all(at.parent().expect("有上级")).expect("建得了");
        std::os::unix::fs::symlink(home.root.path().join(target), at).expect("建得了链接");
    };
    link("token.txt", "scripts/token.py");
    link("nowhere.py", "scripts/gone.py");
    link("keys", "skills/keys");
    link("token.txt", "commands/token.md");
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
            ("script", "scripts/gone.py", "script_skipped"),
            ("script", "scripts/token.py", "script_link_outside"),
            ("skill", "skills/keys", "skill_link_outside"),
            ("slash", "commands/token.md", "slash_link_outside"),
        ],
        "{reply}"
    );
    assert!(problems[0].3.contains("broken link"), "{reply}");
    for (n, part) in [(1, "token.txt"), (2, "keys"), (3, "token.txt")] {
        assert!(problems[n].3.contains(part), "说指到了哪一片：{reply}");
    }
}

/// 留下的状态目录（施工 X-44）：家目录里删了的脚本、没装的包的脚本（同名的家目录脚本在也算），状态目录还在的各报一条
/// `script_state_left`，带目录；脚本还在的不报。
#[tokio::test]
async fn check_reports_state_left_by_scripts_that_are_gone() {
    let home = Home::new();
    home.write("scripts/weather.py", &echo("weather"));
    for kept in ["~home/weather", "~home/gone", "kit/live", "kit/weather"] {
        home.write(&format!("state/scripts/{kept}/data"), "x");
    }
    let mut client = Client::connect(core(&home, &Script::new([])));
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let left: Vec<(String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["code"] == "script_state_left")
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("file"), text("message"))
        })
        .collect();
    assert_eq!(left.len(), 3, "{reply}");
    assert_eq!(left[0].0, "state/scripts/kit/live");
    assert!(left[0].1.contains("kit/live"), "{reply}");
    assert_eq!(left[1].0, "state/scripts/kit/weather");
    assert_eq!(left[2].0, "state/scripts/~home/gone");
}

/// 没登记上的脚本分三种（施工 X-46）：撞了别的工具名字的 `script_clashed`，造不成工具的 `script_not_made`（带为什么），核心
/// 起来以后才放进去、还没读到的 `script_not_loaded`（这个核心不跟着家目录）。
#[tokio::test]
async fn unregistered_scripts_say_why() {
    let home = Home::new();
    home.write("scripts/read.py", &echo("read"));
    home.write(&format!("scripts/{UNMADE}.py"), &echo(UNMADE));
    let core = core(&home, &Script::new([]));
    home.write("scripts/later.py", &echo("later"));
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let problems: Vec<(String, String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("file"), text("code"), text("message"))
        })
        .collect();
    let code = |file: &str| {
        problems
            .iter()
            .find(|(at, _, _)| at == file)
            .map(|(_, code, message)| (code.as_str(), message.as_str()))
            .unwrap_or_else(|| panic!("{file} 没报：{reply}"))
    };
    assert_eq!(code("scripts/read.py").0, "script_clashed");
    let (unmade, why) = code(&format!("scripts/{UNMADE}.py"));
    assert_eq!(unmade, "script_not_made");
    assert!(why.contains(&format!("Cannot make {UNMADE}.")), "{why}");
    let (later, said) = code("scripts/later.py");
    assert_eq!(later, "script_not_loaded");
    assert!(said.contains("later"), "{said}");
    assert_eq!(problems.len(), 3, "{reply}");
}
