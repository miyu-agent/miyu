//! 这一块进的是叫醒那一轮消息的尾巴，以后原样固化（施工 LF-6，`docs/blueprint/memory.md` 第三条第 9 款；核心主会话审内核接口时
//! 补的）：带了它的那一次请求发出去以后，同一个会话下一轮的请求照它逐字节开头，缓存接得上。

use super::*;

/// 编成 JSON 的字节。
fn json<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("编得成 JSON")
}

#[tokio::test]
async fn the_next_turn_starts_with_the_request_that_carried_the_block_byte_for_byte() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let pad = save(&home, "群友换了手柄", &[&other], alice(), None);
    wake(&handle, "cmd-2").await;
    wake(&handle, "cmd-3").await;
    let requests = script.requests();
    let (woken, next) = (&requests[1].1, &requests[2].1);
    let block = format!("<memories>\n{}\n</memories>\n", line(pad, "群友换了手柄"));
    assert!(
        blocks(woken).contains(&block),
        "叫醒的那一次带了这一块：{:?}",
        blocks(woken)
    );

    assert_eq!(json(&next.system), json(&woken.system), "system 一样");
    assert_eq!(json(&next.tools), json(&woken.tools), "工具面一样");
    assert!(next.messages.len() > woken.messages.len(), "下一轮接着往下");
    for (n, (sent, again)) in woken.messages.iter().zip(&next.messages).enumerate() {
        assert_eq!(json(again), json(sent), "第 {n} 条消息逐字节一样");
    }
    stop(&handle).await;
}
