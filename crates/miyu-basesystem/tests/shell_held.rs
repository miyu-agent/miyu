//! 命令里调的工具等人确认的那几段不算时限（施工 M-7 上，`tools/shell.md`「结果」）：真的 `shell`，三个平台都跑。假端口一拿到
//! 「在等人」的口子就说在等、4 秒以后说等完了；命令睡 2 秒、时限 1 秒，照常跑完。没交口子、没扣掉的话，1 秒就杀了。等人的那段
//! 留得长：Windows 上 PowerShell 起来就要将近 1 秒（CI 上 1.5 秒的时候，睡完了还没退出就到时了）。

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::event::NestedStatus;
use miyu_kernel::id::JobId;
use miyu_tool::{Call, Calling, Hold, Nested, NestedPort, Schema, Settled};

use crate::support::{Site, tool};

/// 拿到口子就说在等、过 4 秒说等完了的端口；调什么都答已取消（这里不调）。
struct Asking;

impl NestedPort for Asking {
    fn call(&self, _name: String, _args: String) -> Calling<'_> {
        Box::pin(async { Nested::Settled(Settled::without_result(NestedStatus::Cancelled, None)) })
    }

    fn adopt(&self, _job: JobId) {}

    fn callable(&self) -> Vec<String> {
        Vec::new()
    }

    fn schema(&self, _name: &str) -> Option<Schema> {
        None
    }

    fn hold(&self, tell: Hold) {
        thread::spawn(move || {
            tell(true);
            thread::sleep(Duration::from_millis(4000));
            tell(false);
        });
    }
}

#[tokio::test]
async fn time_spent_waiting_for_someone_is_not_counted_against_the_limit() {
    let site = Site::new();
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 2; Write-Output slept"
    } else {
        "sleep 2; echo slept"
    };
    let call = Call {
        args: json!({ "command": command, "description": "Test", "timeout": 1000 }).to_string(),
        cwd: site.0.join("work").to_string_lossy().into_owned(),
        nested: Some(Arc::new(Asking)),
        ..miyu_tool::testkit::call("{}")
    };
    let done = tool("shell")
        .run(call, miyu_tool::Progress::new(|_| {}))
        .await;
    let text: String = done
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect();
    assert!(!done.error, "等人的 4 秒不算，命令没被杀：{text}");
    assert!(text.contains("slept"), "{text}");
}
