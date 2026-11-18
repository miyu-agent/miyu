//! 新开会话、敲斜杠命令这条路不吃光线程的栈（施工 LF-12 修；原来走的那一串施工 RM-1 拆了，换成这一串）。
//! 调试版里每一层的状态先在栈上放一份再挪走，一层套一层：新开会话本身在 Linux 上就要 800 KiB 上下，Windows 的 CI 上溢出过
//! （线程默认 2 MiB），把整个测试进程带挂。新开会话（`Sessions::create`）、命令分发（`commands::fresh`）、模板命令、提示词命令
//! 这几步在定义处就装箱（`sessions::Heaped`），调的一方那一帧里只放一个指针。
//!
//! 这里在一条 1.5 MiB 栈的线程上走一遍：连接、握手、新开会话、敲两条命令。溢出是整个进程退出，不是一条测试失败。

use std::sync::Arc;

use serde_json::json;

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 线程的栈：比默认的 2 MiB 小，留了余量给帧大一些的平台。
const STACK: usize = 1536 * 1024;

/// 连接、握手、新开会话，敲 `/loop`、`/loop off`。
async fn walk() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(1_792_972_800_000);
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    let tools = Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法");
    let core = core_with_tools(&home, &script, &hand, tools);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (id, text) in [("k1", "/loop 5m 看看 CI"), ("k2", "/loop off")] {
        let reply = client
            .call(id, "command.run", json!({"session": session, "text": text}))
            .await;
        assert!(reply["result"]["said"].is_string(), "{reply}");
    }
}

#[test]
fn opening_a_session_and_running_commands_fit_in_a_small_stack() {
    let walked = std::thread::Builder::new()
        .name("small-stack".to_string())
        .stack_size(STACK)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("起得了运行时")
                .block_on(walk());
        })
        .expect("起得了线程")
        .join();
    assert!(walked.is_ok(), "走完了");
}
