//! 哪些请求在后台答（施工 8-20 补、V-6 上、7-4 修）：`job.stop` 要等子代理停完，在后台答，不挡同一条连接后面的请求。

use std::sync::Arc;

use miyu_kernel::id::{AccountId, CommandId};
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;
use serde_json::json;

use super::answered_later;
use crate::Core;
use crate::test_support::{resources, temp_root};
use crate::wire::Request;

#[test]
fn stopping_a_job_is_answered_in_the_background() {
    let (root, _) = temp_root("later");
    let core = Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    );
    let request = |method: &str| Request {
        id: CommandId::parse("c1").expect("合写法"),
        method: method.to_string(),
        params: json!({"session": "s", "job": "j1"}),
        system: false,
    };
    assert!(answered_later(&core, &request("job.stop")));
    assert!(answered_later(&core, &request("model.call")));
    assert!(!answered_later(&core, &request("session.list")));
}
