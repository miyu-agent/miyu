//! 「她现在在干嘛」的测试用的（施工 LF-7）：带循环、子代理工具、场所配置、手拨的钟的核心；会话日志里交过的那几块；人生那一行。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Clock, Loop, Schedule, Target};
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use super::clock::Hand;
use super::venues::BINDINGS;
use super::{Home, TOKEN, default_resources};

/// 钟从这一刻起：2026-10-26T00:00:00Z。
pub const START: i64 = 1_792_972_800_000;

/// 循环的工具和基础工具（子代理）。
fn catalog() -> Catalog {
    let resources = default_resources();
    Catalog::in_packages([
        (
            miyu_schedule::LOOP,
            miyu_schedule::loop_tools(&resources).expect("出厂的字"),
        ),
        (
            miyu_tool::BASESYSTEM,
            miyu_basesystem::tools(&resources).expect("出厂的字"),
        ),
    ])
    .expect("合写法")
}

/// 照数据根里的配置（默认人格软件工程师、`qq:10001` 是主人）起来的核心，唤醒表照 `hand` 看时间，沙盒当能用。
pub fn core(home: &Home, script: &Script, hand: &Arc<Hand>) -> Arc<Core> {
    home.write(
        "etc/config.toml",
        &format!("[persona]\ndefault = \"engineer\"\n{BINDINGS}"),
    );
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::CommandSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
        miyu_session::loading::ITEMS,
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
    let core = Arc::new(
        home.core_full(script, catalog(), None, TOKEN)
            .with_config(config)
            .with_clock(clock)
            .with_sandbox(miyu_sandbox::Availability::Usable(PathBuf::from(
                "miyu-sandbox",
            ))),
    );
    core.start_schedules();
    core
}

/// 她调 `schedule_wakeup`：十分钟后醒，`reason` 写了的带上。
pub fn wakeup(reason: Option<&str>) -> Play {
    let args = match reason {
        Some(reason) => serde_json::json!({"delay_seconds": 600, "reason": reason}),
        None => serde_json::json!({"delay_seconds": 600}),
    };
    Play::calls(&[("schedule_wakeup", &args.to_string())])
}

/// 会话日志里交过的「在干嘛」那几块：原文和记号。
pub fn doings(home: &Home, session: &str) -> Vec<(String, Vec<String>)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match (&event.by, event.body) {
            (By::Module(module), Body::ContextInjected(fact))
                if module.id.as_str() == "life" && fact.kind.as_str() == "doing" =>
            {
                Some((fact.text, fact.refs))
            }
            _ => None,
        })
        .collect()
}

/// 人生那一行（跑在 `session` 上的）。
pub fn life_row(core: &Core, session: &str) -> Schedule {
    core.schedules()
        .list()
        .expect("读得了")
        .into_iter()
        .find(|one| {
            one.looping == Some(Loop::Life)
                && matches!(&one.target, Target::Session { session: on } if on.as_str() == session)
        })
        .expect("有人生那一行")
}

/// 写好的一行：`since`、`until` 是毫秒，照这台机器这一刻的时区写（会话的时区是造会话时照它定的，`sessions.rs` 的 `offset`）。
pub fn shell(since: i64, until: i64, text: &str) -> String {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    let offset = UtcOffset::from_minutes(minutes).expect("在时区的范围里");
    let local = |ms: i64| {
        Timestamp::from_unix_millis(ms)
            .expect("在时刻的范围里")
            .local_minute(offset)
    };
    format!(
        "<life-doing since=\"{}\" until=\"{}\">{text}</life-doing>\n",
        local(since),
        local(until)
    )
}
