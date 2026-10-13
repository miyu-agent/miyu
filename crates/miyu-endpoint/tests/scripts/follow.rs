//! 家目录里的脚本跟着变（施工 X-12 补，2026-10-13 项目主人照推荐定）：核心在跑时往管理员家目录的 `scripts/` 里放一个，当场
//! 登记成工具，下一轮的工具面里有它；删了，当场拿掉。不用重启核心、不用装卸包。

use std::time::Duration;

use super::*;

/// 等登记上的脚本正好是 `wanted`（照名字排），最多等六十秒（test-waits 再补）。
async fn until_scripts(core: &Core, wanted: &[&str]) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let names: Vec<String> = core
            .scripts()
            .iter()
            .map(|one| one.head.name.clone())
            .collect();
        if names == wanted {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等到的是 {names:?}，要的是 {wanted:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_script_put_in_or_taken_out_of_the_home_folder_is_followed_at_once() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let core = core(&home, &script);
    assert!(core.scripts().is_empty(), "起来时家目录里还没有");
    let _watching = core.watch_scripts().expect("看得了");
    assert!(
        home.root.path().join("home/alice/scripts").is_dir(),
        "没有的先建上"
    );
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    until_scripts(&core, &["weather"]).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello_without_input().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    let tools: Vec<String> = script.requests()[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    assert_eq!(tools, ["read", "weather"], "新放的进了工具面");

    std::fs::remove_file(home.root.path().join("home/alice/scripts/weather.py")).expect("删得掉");
    until_scripts(&core, &[]).await;
    client.say("c2", &session, "再来").await;
    home.until_turns(&session, 2).await;
    let tools: Vec<String> = script.requests()[1]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    assert_eq!(tools, ["read"], "删了的下一轮就没有了");
}
