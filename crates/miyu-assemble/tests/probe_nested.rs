//! 请求形状探针：脚本里的 `miyu call`（施工 M-1，设计 `39-脚本调工具.md` 第七节第 2 条）。真内核、真组装照同一份剧本跑两遍：
//! 一遍她的调用跑着的时候（这张脸的工具面上没有 `shell`，用 `read` 当父调用：内核不看父级是哪件工具）脚本调了几次工具（`tool.nested`），一遍没调；每一次请求逐字节一样，嵌套调用不进上下文。

use crate::support::stage;
use miyu_kernel::testkit::{Line, Play, Stage};

/// 剧本：她调一次工具，它跑着的时候脚本调了 `calls` 次 `read`，结果回来她说一句；人再问一句。
fn script(calls: usize) -> Stage {
    let mut s = stage();
    s.model([
        Line::calls("我数一下。", &[("read", r#"{"path":"TODO"}"#)]),
        Line::says("数完了：一共 12 处。"),
        Line::says("好。"),
    ]);
    s.tools([Play::done("12\n").held()]);
    s.say("数一下 TODO 有几处");
    let parent = s.held_tools()[0];
    for n in 1..=calls {
        s.nested_call(&format!("{parent}#{n}"), "read", r#"{"path":"a"}"#);
        s.nested_done(&format!("{parent}#{n}"), false);
    }
    s.release_tool(parent);
    s.say("谢谢");
    s
}

#[test]
fn nested_calls_leave_every_request_as_it_was() {
    let plain = script(0);
    let scripted = script(3);
    let kinds = |s: &Stage| {
        s.log()
            .iter()
            .filter(|e| e.body.kind() == "tool.nested")
            .count()
    };
    assert_eq!((kinds(&plain), kinds(&scripted)), (0, 3));
    let requests = |s: &Stage| -> Vec<_> { s.requests().iter().map(|(_, r)| r.clone()).collect() };
    assert_eq!(requests(&plain).len(), 3);
    assert_eq!(requests(&plain), requests(&scripted));
}
