//! 开会话、说一轮不吃栈（施工 J-1 上修二）：造会话、载入的 future 有几十 KB，没装箱时每个调用方的 future 都跟着大，调试构建在
//! Windows 较深的调用链上栈溢出（J-1 上的 CI 在 `route_learned` 上撞到过）。在一条只有 1 MB 栈的线程上跑一遍，撑大了先在这里红。

use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 测试线程给多大的栈：Windows 主线程的默认大小。
const STACK: usize = 1024 * 1024;

#[test]
fn opening_a_session_and_talking_fits_in_a_small_stack() {
    let ran = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("造得出运行时");
            runtime.block_on(async {
                let home = Home::new();
                let script = Script::new([Play::Says("在。")]);
                let handle = home.create(&script).await;
                let said = ask(&handle, "m1", say("在吗")).await;
                assert!(said.is_ok(), "{said:?}");
            });
        })
        .expect("起得了线程")
        .join();
    assert!(ran.is_ok(), "1 MB 的栈不够用");
}
