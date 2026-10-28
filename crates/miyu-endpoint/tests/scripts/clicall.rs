//! 脚本里能不能调（施工 M-2，设计 `39-脚本调工具.md` 第三节）：脚本工具开头写了 `# Clicall: yes` 的能，没写的、写了 `no` 的
//! 不能（施工 M-2 三补，原来没写的能）；个人设置写了 `clicall.tools.<名字>` 的盖过来处，之后开的会话照它，记进策略快照。

use miyu_policy::Snapshot;
use miyu_store::blob::Blobs;

use super::*;

/// 会话的策略快照里，工具名到脚本里能不能调。
fn clicall(home: &Home, session: &str) -> Vec<(String, bool)> {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    let snapshot = Snapshot::from_bytes(&bytes).expect("读得懂");
    snapshot
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), tool.clicall))
        .collect()
}

#[tokio::test]
async fn scripts_are_callable_only_when_they_or_the_settings_say_yes() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
    home.write(
        "home/alice/scripts/lookup.py",
        &echo("lookup").replace("# Need load: no\n", "# Need load: no\n# Clicall: yes\n"),
    );
    home.write(
        "home/alice/scripts/secret.py",
        &echo("secret").replace("# Need load: no\n", "# Need load: no\n# Clicall: no\n"),
    );
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(core(&home, &script));
    client.hello_without_input().await;
    let work = home.work.to_string_lossy().into_owned();
    let params = json!({"cwd": work, "preset": "full"});
    let first = session(&mut client, &home, "c1", params.clone()).await;
    assert_eq!(
        clicall(&home, &first),
        [
            ("lookup".to_string(), true),
            ("read".to_string(), false),
            ("secret".to_string(), false),
            ("weather".to_string(), false),
        ],
        "脚本写了 yes 的能，没写的、写了 no 的不能；自带的照它登记的"
    );

    let set = client
        .call(
            "s1",
            "config.set",
            json!({"layer": "system", "changes": [
                {"key": "clicall.tools.weather", "value": true},
                {"key": "clicall.tools.lookup", "value": false},
                {"key": "clicall.tools.read", "value": true},
            ]}),
        )
        .await;
    assert_eq!(
        set["result"]["keys"]["clicall.tools.weather"]["applies"], "next_turn",
        "{set}"
    );
    let second = session(&mut client, &home, "c2", params).await;
    assert_eq!(
        clicall(&home, &second),
        [
            ("lookup".to_string(), false),
            ("read".to_string(), true),
            ("secret".to_string(), false),
            ("weather".to_string(), true),
        ],
        "人改的盖过来处：没写的能打开，写了 yes 的能关掉"
    );
}
