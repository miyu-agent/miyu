//! fork 子代理的记忆照子代理、派普通子代理照深度上限（施工 A-F 中）。

use super::*;

/// 记忆照子代理（施工 A-F 中）：工具面上照父会话有记忆的几件（快照是同一份），可 fork 子代理不召回、不记，调了也记不下。
#[tokio::test]
async fn a_fork_does_not_remember_like_a_subagent() {
    let home = Home::new();
    let forks = Forks::new(&home);
    let script = Script::new([
        subagent(serde_json::json!({"fork": true})),
        Play::Says("派出去了。"),
        Play::calls(&[("remember", r#"{"class":"user","text":"团子爱吃鸡胸肉"}"#)]),
        Play::Says("好。"),
    ]);
    let resources = home.resources.path();
    let tools = Catalog::in_packages(vec![
        (
            miyu_tool::BASESYSTEM,
            miyu_basesystem::tools(resources).expect("读得出"),
        ),
        (
            miyu_memory::PACKAGE,
            miyu_memory::tools(resources).expect("读得出"),
        ),
    ])
    .expect("合写法");
    let lines = Lines {
        sessions: Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    turn(&home, &parent, 1).await;
    let (_, child) = forks.forked().remove(0);
    let (_, id, by, command) = forks.sent().remove(0);
    let branch = home
        .load_full(
            &child,
            &script,
            &tools,
            &environment().cwd,
            Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        )
        .await;
    within("交代的回应", branch.command(id, by, command))
        .await
        .expect("子会话在跑");
    let child_log = until_logged(&home, &child, |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= 2
    })
    .await;
    let said = text(results(&child_log).last().expect("有结果"));
    assert!(!said.contains("Saved as"), "fork 子代理记不下：{said}");
    let requests = script.requests();
    let names = |n: usize| -> Vec<String> {
        requests[n]
            .1
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect()
    };
    assert!(
        names(2).contains(&"remember".to_string()),
        "工具面照父会话的"
    );
    stop(&branch).await;
    stop(&parent).await;
}

/// fork 子代理派普通子代理照深度上限（施工 A-F 中）：父会话是第 1 层的子代理，fork 出来的是第 2 层，工具面照父会话的还有
/// `subagent`，派的时候挡，会话表一个都没造。会话层的分叉也不许从 fork 子代理再分。
#[tokio::test]
async fn a_fork_at_the_depth_limit_starts_no_subagent() {
    let home = Home::new();
    let forks = Forks::new(&home);
    let script = Script::new([
        subagent(serde_json::json!({"fork": true})),
        Play::Says("派出去了。"),
        subagent(serde_json::json!({})),
        Play::Says("自己做。"),
    ]);
    let tools = basesystem(&home);
    let lines = Lines {
        sessions: Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        lineage: Some(Lineage {
            parent: SessionId::parse(crate::support::table::OTHER).expect("合写法"),
            depth: 1,
            workspace_from: None,
        }),
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    turn(&home, &parent, 1).await;
    let (asked, child) = forks.forked().remove(0);
    assert_eq!(asked.lineage.depth, 2);
    let (_, id, by, command) = forks.sent().remove(0);
    let branch = home
        .load_full(
            &child,
            &script,
            &tools,
            &environment().cwd,
            Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        )
        .await;
    within("交代的回应", branch.command(id, by, command))
        .await
        .expect("子会话在跑");
    let child_log = until_logged(&home, &child, |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= 2
    })
    .await;
    assert_eq!(
        text(results(&child_log).last().expect("有结果")),
        "The subagent could not be started.\n"
    );
    assert!(
        forks
            .made
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty(),
        "到了上限，一个都没造"
    );
    let again = miyu_session::fork(Fork {
        root: &home.root,
        parent: &child,
        id: new_id(now()),
        cause: CommandId::parse("f1").expect("合写法"),
        by: alice(),
        at: now(),
        index: None,
        seal_after: None,
        within: Some(Within {
            call: subagent_call(&child_log),
            depth: 3,
            placeholder: PLACEHOLDER.to_string(),
        }),
    })
    .await;
    assert!(
        matches!(again, Err(miyu_session::ForkError::NotMain)),
        "fork 子代理不能再分：{again:?}"
    );
    stop(&branch).await;
    stop(&parent).await;
}
