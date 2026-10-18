//! 设置页的下拉里选项后面写这一刻的状态（施工 N-2 下，设计 36 第三节第 1 条）：内置包的端口说哪个选项冷却到什么时候，
//! `config.schema` 照连接的语言、核心这台机器的时区写成「冷却到 时:分」；别的选项不写，没装端口的核心也不写。

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde_json::json;

use miyu_config::Item;
use miyu_endpoint::builtins::{Builtins, Group, OptionState};
use miyu_session::testkit::Script;
use miyu_store::packages::Found;
use miyu_tool::Catalog;

use crate::support::*;

/// 说 `ui.language` 的 `en` 冷却着的端口（拿一个现成的选项项当搜索来源那样用）。
struct Cooling(SystemTime);

impl Builtins for Cooling {
    fn tools(&self, _: &[Found]) -> Result<Vec<Group>, String> {
        Ok(Vec::new())
    }

    fn settings(&self, _: &mut [Found]) -> Vec<Item> {
        miyu_endpoint::settings::UiSettings::ITEMS.to_vec()
    }

    fn embed(&self, _: &[&Found]) -> Option<miyu_session::EmbedSetup> {
        None
    }

    fn script(
        &self,
        _: &miyu_policy::script::ScriptHead,
        _: std::path::PathBuf,
    ) -> Result<Arc<dyn miyu_tool::Tool>, String> {
        Err("no scripts here".to_string())
    }

    fn option_state(&self, key: &str, option: &str) -> Option<OptionState> {
        (key == "ui.language" && option == "en").then_some(OptionState::CoolingUntil(self.0))
    }
}

/// `config.schema` 里 `ui.language` 的选项：值和暗字。
async fn notes(client: &mut Client) -> Vec<(String, Option<String>)> {
    let reply = client
        .call("s", "config.schema", json!({"keys": ["ui.language"]}))
        .await;
    reply["result"]["items"][0]["options"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|option| {
            (
                option["value"].as_str().unwrap_or_default().to_string(),
                option["note"].as_str().map(str::to_string),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_cooling_option_says_until_when_in_the_connections_language() {
    let home = Home::new();
    let until = SystemTime::now() + Duration::from_secs(90 * 60);
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_builtins(Arc::new(Cooling(until))),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let got = notes(&mut client).await;

    let en: Vec<&Option<String>> = got
        .iter()
        .filter(|(value, _)| value == "en")
        .map(|(_, note)| note)
        .collect();
    let note = en[0]
        .as_deref()
        .unwrap_or_else(|| panic!("en 有暗字：{got:?}"));
    let clock = note
        .strip_prefix("冷却到 ")
        .unwrap_or_else(|| panic!("中文的连接写中文：{note}"));
    let digits: Vec<bool> = clock.chars().map(|c| c.is_ascii_digit()).collect();
    assert_eq!(digits, [true, true, false, true, true], "时:分：{clock}");
    assert!(
        got.iter()
            .filter(|(value, _)| value != "en")
            .all(|(_, note)| note.is_none()),
        "别的选项不写：{got:?}"
    );
}

#[tokio::test]
async fn a_core_without_the_port_writes_no_state() {
    let home = Home::new();
    let core = Arc::new(home.core_full(&Script::new([]), Catalog::default(), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let got = notes(&mut client).await;

    assert!(got.iter().all(|(_, note)| note.is_none()), "{got:?}");
}
