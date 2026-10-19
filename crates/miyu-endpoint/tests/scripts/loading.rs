//! 脚本的加载方式（施工 L-3，设计 `25-工具加载.md` W9，2026-10-20 项目主人改）：开头没写 `Need load` 的先加载，工具面上是外壳、
//! 多一件 `load_tools`；个人设置写了 `loading.tools.<名字> = false` 的，之后开的会话全量。

use super::*;

/// 工具面上的几件：名字和参数。
fn face(script: &Script, n: usize) -> Vec<(String, String)> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), tool.parameters.get().to_string()))
        .collect()
}

#[tokio::test]
async fn a_script_loads_on_demand_unless_settings_say_otherwise() {
    let home = Home::new();
    home.write(
        "home/alice/scripts/weather.py",
        &echo("weather").replace("# Need load: no\n", ""),
    );
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(core(&home, &script));
    client.hello_without_input().await;
    let work = home.work.to_string_lossy().into_owned();
    session(
        &mut client,
        &home,
        "c1",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    let first = face(&script, 0);
    let names: Vec<&str> = first.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["load_tools", "read", "weather"], "没写的先加载");
    let weather = first
        .iter()
        .find(|(name, _)| name == "weather")
        .expect("有");
    assert_eq!(weather.1, r#"{"type":"object"}"#, "只给外壳");

    let set = client
        .call(
            "s1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "loading.tools.weather", "value": false}]}),
        )
        .await;
    assert_eq!(
        set["result"]["keys"]["loading.tools.weather"]["applies"], "next_turn",
        "{set}"
    );
    session(
        &mut client,
        &home,
        "c2",
        json!({"cwd": work, "preset": "full"}),
    )
    .await;
    let second = face(&script, 1);
    let names: Vec<&str> = second.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["read", "weather"], "人改成全量的照全量");
    let weather = second
        .iter()
        .find(|(name, _)| name == "weather")
        .expect("有");
    assert!(weather.1.contains("\"say\""), "完整的参数：{}", weather.1);
}
