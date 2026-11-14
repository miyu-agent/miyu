//! 经端口交出去的请求里的供应商侧工具（施工 HT-2）：别家的在 `driver.call` 的 `request` 里是一块 `text`，自己家的原样。
//! 换成什么字在 `wire/tests/hosted.rs` 里逐块查，这里只查走的是会话冻结的那份占位。

use super::*;

pub(super) use crate::route::extension::wire::tests::hosted::texts_with;

/// 带着出厂的供应商侧工具那四份的占位。
pub(super) fn texts() -> DriverTexts {
    texts_with(true)
}

#[tokio::test]
async fn hosted_tools_reach_the_extension_retold_unless_they_are_its_own() {
    use crate::route::extension::wire::tests::hosted::hosted;
    let fake = scripted(vec![Plan::Answer(hello(), Duration::ZERO, usage())]);
    let mut request = request(None);
    request.messages.insert(
        0,
        Message::Assistant {
            blocks: vec![hosted("call_1_1", Some("pi/ag")), hosted("call_1_2", None)],
        },
    );
    let (ended, _, _) = run(Arc::clone(&fake), Duration::from_secs(5), request).await;
    assert_eq!(error(&ended), None);
    let params = fake.params.lock().expect("锁")[0].clone();
    let blocks = &params["request"]["messages"][0]["blocks"];
    assert_eq!(blocks[0]["type"], "hosted_tool");
    assert_eq!(blocks[0]["private"]["data"]["id"], "srvtoolu_1");
    assert_eq!(blocks[1]["type"], "text");
    let text = blocks[1]["text"].as_str().expect("一段字");
    assert!(
        text.starts_with("<hosted-tool name=\"web_search\">\n"),
        "{text}"
    );
    assert!(text.ends_with("</hosted-tool>"), "末尾不带换行：{text}");
    assert!(
        text.contains("\n{\"query\":\"<rust>\"}\n"),
        "参数原样：{text}"
    );
}

/// 一块供应商侧工具 `name`：开、参数、私有数据、结果、收；再一块正文。
fn searched(name: &str) -> Vec<Value> {
    vec![
        json!([{"start": {"index": 0, "kind": {"hosted_tool": {"name": name}}}},
               {"text": {"index": 0, "text": "{\"query\":\"rust\"}"}}]),
        json!([{"private": {"index": 0, "data": {"step": 1}}},
               {"outcome": {"index": 0, "text": "1. Rust"}},
               {"end": {"index": 0}}]),
        json!([{"start": {"index": 1, "kind": "text"}},
               {"text": {"index": 1, "text": "found it"}},
               {"end": {"index": 1}}]),
    ]
}

/// 照剧本发一次，交回结尾、收到的增量、模型资料（看说过哪些没声明的名字）。
async fn run_kept(batches: Vec<Value>) -> (Exchanged, Vec<Delta>, Arc<ModelData>) {
    let fake = scripted(vec![Plan::Answer(batches, Duration::ZERO, usage())]);
    let ready = ready(&fake, Duration::from_secs(5));
    let Speaker::Extension(reached) = &ready.speaker else {
        unreachable!()
    };
    let blobs = Blobs::new(std::env::temp_dir().join("miyu-ext-driver-unused"));
    let mut deltas = Vec::new();
    let ended = exchange(
        &ready,
        reached,
        Cow::Owned(request(None)),
        &blobs,
        std::future::pending(),
        |progress| {
            if let Progress::Delta(delta) = progress {
                deltas.push(delta);
            }
        },
    )
    .await;
    (ended, deltas, Arc::clone(&ready.learn.data))
}

#[tokio::test]
async fn a_hosted_tool_comes_through_with_its_outcome_and_the_extension_s_private_data() {
    let (ended, deltas, data) = run_kept(searched("search_web")).await;
    assert_eq!(error(&ended), None);
    let mut accumulator = miyu_kernel::accumulate::Accumulator::default();
    for delta in deltas {
        accumulator.apply(delta).expect("查过了的");
    }
    let blocks = accumulator.finish(miyu_kernel::id::Seq::new(1).expect("序号"));
    let Block::HostedTool(hosted) = &blocks[0] else {
        panic!("{blocks:?}")
    };
    assert_eq!(hosted.name, "search_web");
    assert_eq!(hosted.args, "{\"query\":\"rust\"}");
    let outcome = hosted.outcome.as_ref().expect("有结果");
    assert_eq!((outcome.text.as_str(), outcome.failed), ("1. Rust", false));
    let private = hosted.private.as_ref().expect("有私有数据");
    assert_eq!(private.driver.as_str(), "pi/ag", "记成扩展驱动的家族");
    assert_eq!(
        data.wires().undeclared("pi/ag", "search_web"),
        Undeclared::Tell,
        "声明了的不说，表里没有它"
    );
}

#[tokio::test]
async fn an_undeclared_hosted_tool_is_taken_and_told_once() {
    let (ended, deltas, data) = run_kept(searched("run_command")).await;
    assert_eq!(error(&ended), None, "没声明的照收");
    assert_eq!(deltas.len(), 8, "一条不少");
    assert_eq!(
        data.wires().undeclared("pi/ag", "run_command"),
        Undeclared::Told,
        "说过了：再来一次不再说"
    );
    assert_eq!(
        data.wires().undeclared("other/ag", "run_command"),
        Undeclared::Tell,
        "别的驱动各说各的"
    );
}

/// 第 `index` 块：只有开、收的供应商侧工具 `name`。
fn bare(index: usize, name: &str) -> Value {
    json!([{"start": {"index": index, "kind": {"hosted_tool": {"name": name}}}},
           {"end": {"index": index}}])
}

/// 日志行这里不查：同一处 `warn!` 别的测试线程也走，线程上装的订阅者收不全（`miyu-endpoint` 另起测试程序查日志行就是为这个）；
/// 记哪一行照 `wires.rs` 交回的说法，查走完以后表里是什么样。
#[tokio::test]
async fn undeclared_names_past_the_cap_are_taken_and_told_once_more() {
    use crate::route::extension::wires::UNDECLARED_PER_DRIVER;
    use miyu_config::package::HOSTED_NAME_BYTES;
    let long = "x".repeat(HOSTED_NAME_BYTES + 1);
    let mut names: Vec<String> = (0..UNDECLARED_PER_DRIVER + 2)
        .map(|n| format!("tool_{n}"))
        .collect();
    names.extend([
        format!("tool_{UNDECLARED_PER_DRIVER}"),
        "tool_0".to_string(),
        long.clone(),
        "search_web".to_string(),
    ]);
    let batches = names
        .iter()
        .enumerate()
        .map(|(index, name)| bare(index, name))
        .collect();
    let (ended, deltas, data) = run_kept(batches).await;
    assert_eq!(error(&ended), None, "没声明的、超了的都照收");
    assert_eq!(deltas.len(), names.len() * 2, "一条不少");
    let mut wires = data.wires();
    for n in 0..UNDECLARED_PER_DRIVER {
        assert_eq!(
            wires.undeclared("pi/ag", &format!("tool_{n}")),
            Undeclared::Told,
            "前 {n} 个进了表、说过了"
        );
    }
    for name in [
        format!("tool_{UNDECLARED_PER_DRIVER}"),
        long,
        "tool_x".to_string(),
    ] {
        assert_eq!(
            wires.undeclared("pi/ag", &name),
            Undeclared::ToldMore,
            "满了的、太长的：还有更多说过了"
        );
    }
    assert_eq!(
        wires.undeclared("other/ag", "tool_0"),
        Undeclared::Tell,
        "别的驱动各算各的"
    );
}

#[tokio::test]
async fn misplaced_outcomes_drop_the_call() {
    let open = json!({"start": {"index": 0, "kind": {"hosted_tool": {"name": "search_web"}}}});
    let outcome = json!({"outcome": {"index": 0, "text": "x"}});
    for (batch, why) in [
        (
            json!([{"start": {"index": 0, "kind": "text"}}, outcome]),
            "结果给了正文块",
        ),
        (json!([open, outcome, outcome]), "来两次"),
        (json!([open, {"end": {"index": 0}}, outcome]), "收全以后来"),
        (json!([outcome]), "块还没开"),
    ] {
        let fake = scripted(vec![Plan::Hang(vec![batch])]);
        let (ended, _, _) = run(Arc::clone(&fake), Duration::from_secs(5), request(None)).await;
        let (class, said) = error(&ended).expect(why);
        assert_eq!(class, ErrorClass::Unclassified, "{why}");
        assert!(
            said.starts_with("the driver sent a bad delta: "),
            "{why}：{said}"
        );
        assert_eq!(
            fake.dropped.lock().expect("锁").len(),
            1,
            "{why}：不要了，端口发 driver.cancel"
        );
    }
}
