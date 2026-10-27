//! 脚本的参数格式一律全量（施工 FL-1，2026-10-27 项目主人定，改掉 L-3 的「没写 `Need load` 的先加载」）：开头写了
//! `# Need load: yes` 的也一开始就发完整的参数，工具面上没有 `load_tools`。做约束解码的供应商见了外壳只能发 `{}`。

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
async fn a_script_is_sent_whole_even_when_it_asks_to_load_on_demand() {
    let home = Home::new();
    home.write(
        "home/alice/scripts/weather.py",
        &echo("weather").replace("# Need load: no\n", "# Need load: yes\n"),
    );
    let script = Script::new([Play::Says("好。")]);
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
    assert_eq!(names, ["read", "weather"], "没有 load_tools");
    let weather = first
        .iter()
        .find(|(name, _)| name == "weather")
        .expect("有");
    assert!(weather.1.contains("\"say\""), "完整的参数：{}", weather.1);
}
