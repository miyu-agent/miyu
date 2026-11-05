//! fork 子代理写了只读（施工 SA-1）：子会话的 `session.created` 照父会话的权限、只读开关打开；没写的照父会话。

use super::*;

/// 父会话派一个 fork 子代理，参数另加 `extra`；交回子会话 `session.created` 记的权限。
async fn forked_permission(extra: serde_json::Value) -> miyu_kernel::event::Permission {
    let home = Home::new();
    let forks = Forks::new(&home);
    let mut args = serde_json::json!({"fork": true});
    for (key, value) in extra.as_object().expect("是对象") {
        args[key] = value.clone();
    }
    let script = Script::new([subagent(args), Play::Says("派出去了。")]);
    let lines = Lines {
        sessions: Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &basesystem(&home), Opening::default(), lines)
        .await;
    turn(&home, &parent, 1).await;
    let forked = forks.forked();
    let (_, child) = forked.first().expect("要会话表 fork 了一个");
    let log = until_logged(&home, child, |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::SessionCreated(_)))
    })
    .await;
    stop(&parent).await;
    log.iter()
        .find_map(|event| match &event.body {
            Body::SessionCreated(created) => Some(created.permission.clone()),
            _ => None,
        })
        .expect("有 session.created")
}

#[tokio::test]
async fn a_read_only_fork_starts_read_only() {
    let plain = forked_permission(serde_json::json!({})).await;
    assert!(!plain.read_only, "没写只读的照父会话：{plain:?}");
    let read_only = forked_permission(serde_json::json!({"read_only": true})).await;
    assert!(read_only.read_only, "写了只读的打开：{read_only:?}");
    assert_eq!(read_only.level, plain.level, "常用的那一级照父会话");
}
