//! 客户端门面等回应的时长（施工 CA-1）：`call` 照旧最多等 30 秒，做得久的请求照 `call_waiting` 给的时长等，回应照编号认。
//! 到时报 `no_answer` 的那一支和原来的 `call` 是同一段代码，这里不另造一个不回话的核心去量它。

use std::time::Duration;

use serde_json::json;

use crate::support::{Home, within};
use miyu_client::open::{Core, WAIT};
use miyu_ipc::connect_or_start;

#[tokio::test]
async fn a_call_waits_as_long_as_it_is_told() {
    let home = Home::new();
    let _held = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起核心");
    let mut core = within("本机令牌连上", Core::connect_running(&home.root, "test"))
        .await
        .expect("连得上");
    assert_eq!(WAIT, Duration::from_secs(30), "照旧默认 30 秒");
    let listed = within(
        "等得够久",
        core.call_waiting("w1", "session.list", json!({}), Duration::from_secs(120)),
    )
    .await
    .expect("等得到回应");
    assert!(listed["sessions"].is_array(), "{listed}");
    let again = within("照旧的 call", core.call("w2", "session.list", json!({})))
        .await
        .expect("同一条连接接着用");
    assert_eq!(again, listed, "两次问的一样");
}
