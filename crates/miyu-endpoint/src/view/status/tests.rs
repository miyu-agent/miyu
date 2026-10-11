//! 会话状态的 `goal` 那一格（施工 X-6）：原文、状态、第几轮、设下的时刻、花了多少；不限的不写上限，跑着的不写原因。

use std::num::NonZeroU32;
use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::Script;
use miyu_session::{Goal, GoalState, Tally};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::goal_of;
use crate::Core;
use crate::test_support::{resources, temp_root};

#[test]
fn a_goal_is_shown_with_its_round_and_spending() {
    let (root, dir) = temp_root("goal");
    let core = Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    );
    let mut spent = Tally::default();
    spent.requests = 3;
    spent.usage.output = 24;
    let mut goal = Goal {
        text: "修绿".to_string(),
        limit: None,
        state: GoalState::Running,
        reason: None,
        round: 3,
        since: Timestamp::parse("2026-10-12T01:00:04.000Z").expect("合写法"),
        spent,
    };
    let shown = goal_of(&core, &goal);
    assert_eq!(
        (
            &shown["text"],
            &shown["state"],
            &shown["round"],
            &shown["since"]
        ),
        (
            &serde_json::json!("修绿"),
            &serde_json::json!("running"),
            &serde_json::json!(3),
            &serde_json::json!("2026-10-12T01:00:04.000Z")
        )
    );
    assert_eq!(shown["usage"]["requests"], 3, "{shown}");
    assert!(
        shown.get("limit").is_none() && shown.get("reason").is_none(),
        "{shown}"
    );
    goal.state = GoalState::Blocked;
    goal.reason = Some("要人给 key".to_string());
    goal.limit = NonZeroU32::new(20);
    let shown = goal_of(&core, &goal);
    assert_eq!(
        (&shown["state"], &shown["reason"], &shown["limit"]),
        (
            &serde_json::json!("blocked"),
            &serde_json::json!("要人给 key"),
            &serde_json::json!(20)
        )
    );
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录留着：{error}");
    }
}
