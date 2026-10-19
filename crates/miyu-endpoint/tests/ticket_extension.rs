//! 交接票只给人（接入通讯平台要的）：核心拉起的扩展要票回 `local_only`，不然扩展能换出管理员的登录令牌。

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::support::extensions::{Program, core, install, quick, read, record, steps};
use crate::support::*;

#[tokio::test]
async fn an_extension_cannot_get_a_ticket() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "asker");
    let args = [keep.as_str(), "hello", "call:login.ticket", "wait"];
    install(&home, "asker", &program.name(), "always", &steps(&args));
    let core = core(&home, quick());
    let _client = Client::connect(Arc::clone(&core));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "{}", read(&path));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let recorded = read(&path);
    let refused: Value =
        serde_json::from_str(recorded.lines().nth(2).expect("有第三行")).expect("是 JSON");
    assert_eq!(
        refused["error"]["data"]["reason"], "local_only",
        "扩展要不了票：{refused}"
    );
}
