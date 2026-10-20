//! 脚本里能不能调（施工 M-2，设计 `39-脚本调工具.md` 第三节）：脚本工具开头没写的能，写了 `# Clicall: no` 的不能；个人设置
//! 写了 `clicall.tools.<名字>` 的盖过来处，之后开的会话照它，记进策略快照。

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
async fn scripts_are_callable_unless_they_or_the_settings_say_no() {
    let home = Home::new();
    home.write("home/alice/scripts/weather.py", &echo("weather"));
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
            ("read".to_string(), false),
            ("secret".to_string(), false),
            ("weather".to_string(), true),
        ],
        "脚本没写的能，写了 no 的不能；自带的照它登记的"
    );

    let set = client
        .call(
            "s1",
            "config.set",
            json!({"layer": "personal", "changes": [
                {"key": "clicall.tools.weather", "value": false},
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
            ("read".to_string(), true),
            ("secret".to_string(), false),
            ("weather".to_string(), false),
        ],
        "人改的盖过来处"
    );
}
