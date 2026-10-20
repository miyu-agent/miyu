//! 后台在不在补（施工 R-25：`miyu check` 印「补着」）：起补的那一刻就算在补，补完了不算；`off` 的不起、不算。真的
//! `miyu-embed`、手造的小模型、真记忆日志。

use std::sync::Arc;
use std::time::Duration;

use miyu_session::{Embedder, Keeper, Listener, Turn, Using, Vectors};

use crate::support::meaning::*;
use crate::support::package::tiny_package;
use crate::support::*;

#[tokio::test]
async fn filling_is_on_from_the_start_until_done() {
    let home = Home::new();
    let setup = tiny_package(&home.scratch.0.join("packages").join("embed"));
    let vectors = Arc::new(Vectors::new(Some(Embedder::new(setup)), model_data(&home)));
    assert!(home.memory.give_vectors(Arc::clone(&vectors)));
    save(&home, "我的猫");
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
        owner: alice_account(),
    };
    assert!(!vectors.filling(), "还没起补");
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    keeper.fill(&using);
    assert!(vectors.filling(), "起补的那一刻就算");
    filled(&home, "local:tiny", true, "m1").await;
    for _ in 0..200 {
        if !vectors.filling() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("补完了还算在补");
}
