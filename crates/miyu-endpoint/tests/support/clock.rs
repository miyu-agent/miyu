//! 手拨的钟（施工 J-1 上）：唤醒表的测试不真等。拨一下，睡着的照新的时刻看到没到。

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use tokio::sync::watch;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Clock, Sleep};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::Script;

use super::{Home, TOKEN};

/// 手拨的钟：从 `start` 毫秒起，只在 [`Hand::advance`] 时走。
pub struct Hand {
    now: AtomicI64,
    moved: watch::Sender<i64>,
}

impl Hand {
    /// 停在 `start` 毫秒。
    pub fn at(start: i64) -> Arc<Hand> {
        Arc::new(Hand {
            now: AtomicI64::new(start),
            moved: watch::Sender::new(start),
        })
    }

    /// 往后拨 `ms` 毫秒。
    pub fn advance(&self, ms: i64) {
        let now = self.now.fetch_add(ms, Ordering::SeqCst) + ms;
        self.moved.send_replace(now);
    }

    /// 现在，照毫秒。
    pub fn millis(&self) -> i64 {
        self.now.load(Ordering::SeqCst)
    }
}

impl Clock for Hand {
    fn now(&self) -> Timestamp {
        Timestamp::from_unix_millis(self.millis()).expect("在时刻的范围里")
    }

    fn sleep_until(&self, at: Timestamp) -> Sleep<'_> {
        let mut moved = self.moved.subscribe();
        Box::pin(async move {
            while self.millis() < at.unix_millis() {
                if moved.changed().await.is_err() {
                    return;
                }
            }
        })
    }
}

/// 一份核心，唤醒表照 `hand` 看时间：读个人设置里的预设那几项，和 [`Home::core_with_items`] 一样读磁盘上的配置。
pub fn core_with(home: &Home, script: &Script, hand: &Arc<Hand>) -> Arc<Core> {
    core_with_tools(home, script, hand, miyu_tool::Catalog::default())
}

/// 同 [`core_with`]，工具目录是 `tools`（施工 J-2：`/loop` 只在工具面上有 `schedule_wakeup` 的会话里认）。
pub fn core_with_tools(
    home: &Home,
    script: &Script,
    hand: &Arc<Hand>,
    tools: miyu_tool::Catalog,
) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_session::switches::ITEMS,
        miyu_session::packages_off::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let clock: Arc<dyn Clock> = Arc::clone(hand) as Arc<dyn Clock>;
    Arc::new(
        home.core_full(script, tools, None, TOKEN)
            .with_config(config)
            .with_clock(clock),
    )
}
