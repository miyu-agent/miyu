//! 人生循环醒来那段话的正文（施工 LF-2，设计 `70-推演人生.md` 第三节第 3 条，`schedules.md` 第三节第 4 条）：推演方法、空一行、
//! 她上次留给自己的话（定时表那一行的 `prompt`）；留言是空的只有推演方法。拼好的原样交进会话、记进 `message.user`，重放一字
//! 不差；固定的框是标签，组装时包（`miyu-assemble` 的 `schedule.rs`）。
//!
//! 推演方法照会话的人格的 `prompts/life.md`（几层照人格的叠法，`miyu_store::personas`）；去掉末尾空白是空的、没有这个文件、
//! 会话不带人格的用出厂的 `software/life/method.txt`。到点那一刻读，改了文件下一次醒来就用新的；读盘在阻塞线程里。

use std::sync::Arc;

use crate::Core;

/// 人格是 `persona` 的会话这一次醒来的正文：推演方法接 `note`。
pub(super) async fn said(core: &Arc<Core>, persona: Option<String>, note: &str) -> String {
    let personas = crate::personas::personas(core);
    let resources = core.resources.clone();
    let method = tokio::task::spawn_blocking(move || {
        let own = persona.and_then(|id| match personas.find(&id) {
            Ok(found) => found.life,
            // 人格后来删了、写坏了的照出厂的过，不让她这一次醒不来；照旧记一行。错的原话带路径，不进运行日志。
            Err(_) => {
                tracing::warn!(target: super::TARGET, "persona life method not read");
                None
            }
        });
        match own.filter(|text| !text.trim().is_empty()) {
            Some(text) => text,
            None => resources.life_method().unwrap_or_else(|_| {
                tracing::warn!(target: super::TARGET, "shipped life method not read");
                String::new()
            }),
        }
    })
    .await
    .unwrap_or_else(|error| {
        tracing::warn!(target: super::TARGET, panic = error.is_panic(), "life method task failed");
        String::new()
    });
    body(&method, note)
}

/// 推演方法去掉末尾空白，空一行接留言；空的那一段不写，末尾不留空行。
fn body(method: &str, note: &str) -> String {
    [method.trim_end(), note]
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
