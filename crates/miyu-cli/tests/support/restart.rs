//! 同一个数据根上重启核心（施工 ACP-9：`miyu acp` 在核心重启以后接着用）：照收到停的信号那样停下会话，丢掉监听器和各个连接，
//! 再在同一个数据根上起一份新的。

use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_sandbox::Availability;
use miyu_session::Models;
use miyu_tool::Catalog;

use super::{Home, dirs, resources};

impl Home {
    /// 重启核心：新的一份请求模型照 `models`，工具照 `tools`，沙盒同 [`Home::with_tools`]。
    pub async fn restart(&mut self, models: Arc<dyn Models>, tools: Catalog) {
        self.core.stop_sessions().await;
        self.running.abort();
        // 等它真放下：监听器、锁、各个连接跟着没了，新的一份才拿得到锁。
        let stopped = (&mut self.running).await;
        assert!(
            stopped.as_ref().is_err_and(|error| error.is_cancelled()),
            "停了"
        );
        let opened = miyu_ipc::open(&self.root, &dirs()).expect("再起得来");
        let core = Core::new(
            self.root.clone(),
            resources(),
            models,
            tools,
            None,
            opened.token.clone(),
        )
        .with_sandbox(Availability::Usable(miyu_sandbox::testkit::built_helper()));
        self.core = Arc::new(core);
        self.running = tokio::spawn(miyu_endpoint::run(opened.listener, Arc::clone(&self.core)));
    }
}
