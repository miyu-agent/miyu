//! 不经核心直接连的模型（施工 RC-1）：模型照真发时的驱动和地址，池取第一个用得了的成员，key、地址取不到的说清楚。

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::test_support::{Held, resolved};

/// 档案：DeepSeek 写了驱动和地址，`[npm]` 认三种内置的包；带上裁出来的目录。
fn held() -> Held {
    Held::new(
        json!({
            "npm": {
                "@ai-sdk/openai-compatible": "openai-chat",
                "@ai-sdk/anthropic": "anthropic",
                "@ai-sdk/openai": "openai-responses"
            },
            "providers": {"deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"}}
        }),
        true,
    )
}

/// 照名字取密钥：`stored` 是密钥文件里的，`env` 是环境变量。
fn secrets<'a>(
    stored: &'a [(&'a str, &'a str)],
    env: &'a [(&'a str, &'a str)],
) -> impl Fn(&KeyRef) -> Option<Secret> + 'a {
    let stored: BTreeMap<&str, &str> = stored.iter().copied().collect();
    let env: BTreeMap<&str, &str> = env.iter().copied().collect();
    move |reference| {
        let value = match reference {
            KeyRef::Secret(name) => stored.get(name.as_str()),
            KeyRef::Env(name) => env.get(name.as_str()),
        }?;
        Secret::new(value).ok()
    }
}

fn said(result: Result<Direct, NoModel>) -> String {
    match result {
        Ok(direct) => panic!("应该用不了：{direct:?}"),
        Err(NoModel(said)) => said,
    }
}

#[test]
fn a_known_provider_takes_its_driver_and_address_from_the_profile() {
    let resolved = resolved(
        "[providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n[models]\nchat = \"deepseek/deepseek-flash\"\n",
    );
    let held = held();
    let direct = direct(
        &resolved,
        &held.knowledge(),
        &secrets(&[("deepseek", "sk-one")], &[]),
    )
    .expect("用得了");
    assert_eq!(direct.base_url, "https://api.deepseek.com");
    assert_eq!(direct.model, "deepseek-flash");
    assert_eq!(direct.driver, Driver::OpenAiChat);
    assert_eq!(direct.key.as_ref().map(Secret::expose), Some("sk-one"));
}

#[test]
fn hand_written_values_and_env_references_are_used() {
    let resolved = resolved(
        "[providers.lab]\ndriver = \"anthropic\"\nbase_url = { env = \"LAB_URL\" }\nkey = { env = \"LAB_KEY\" }\n\n[models]\nchat = \"lab/claude-x\"\n",
    );
    let held = held();
    let direct = direct(
        &resolved,
        &held.knowledge(),
        &secrets(
            &[],
            &[("LAB_URL", "http://127.0.0.1:9/v1"), ("LAB_KEY", "sk-lab")],
        ),
    )
    .expect("用得了");
    assert_eq!(direct.base_url, "http://127.0.0.1:9/v1");
    assert_eq!(direct.driver, Driver::Anthropic);
    assert_eq!(direct.key.as_ref().map(Secret::expose), Some("sk-lab"));
}

#[test]
fn a_provider_without_a_key_sends_none() {
    let resolved = resolved(
        "[providers.home]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:11434/v1\"\n\n[models]\nchat = \"home/qwen\"\n",
    );
    let held = held();
    let direct = direct(&resolved, &held.knowledge(), &secrets(&[], &[])).expect("用得了");
    assert_eq!(direct.key, None);
    assert_eq!(direct.base_url, "http://127.0.0.1:11434/v1");
}

#[test]
fn the_model_s_own_package_picks_its_driver_and_the_catalog_gives_the_address() {
    // opencode Go 没写驱动、地址：地址照目录，minimax-m3 自己的包是 anthropic 的（施工 8-14）。
    let resolved = resolved(
        "[providers.opencode-go]\nkey = { secret = \"go\" }\n\n[models]\nchat = \"opencode-go/minimax-m3\"\n",
    );
    let held = held();
    let direct = direct(
        &resolved,
        &held.knowledge(),
        &secrets(&[("go", "sk-go")], &[]),
    )
    .expect("用得了");
    assert_eq!(direct.base_url, "https://opencode.ai/zen/go/v1");
    assert_eq!(direct.driver, Driver::Anthropic);
}

#[test]
fn a_missing_key_or_address_is_said_and_nothing_is_chosen() {
    let held = held();
    let resolved = resolved(
        "[providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n[models]\nchat = \"deepseek/deepseek-flash\"\n",
    );
    assert_eq!(
        said(direct(&resolved, &held.knowledge(), &secrets(&[], &[]))),
        "key secret:deepseek of provider \"deepseek\" is not set"
    );
    let resolved = resolved_with_url_env();
    assert_eq!(
        said(direct(&resolved, &held.knowledge(), &secrets(&[], &[]))),
        "provider \"lab\" has no usable base_url"
    );
}

fn resolved_with_url_env() -> Resolved {
    resolved(
        "[providers.lab]\ndriver = \"openai-chat\"\nbase_url = { env = \"LAB_URL\" }\n\n[models]\nchat = \"lab/m\"\n",
    )
}

#[test]
fn nothing_configured_says_so() {
    let held = held();
    assert_eq!(
        said(direct(&resolved(""), &held.knowledge(), &secrets(&[], &[]))),
        NOT_CONFIGURED
    );
    let resolved = resolved("[models]\nchat = \"nobody/m\"\n");
    assert_eq!(
        said(direct(&resolved, &held.knowledge(), &secrets(&[], &[]))),
        "no provider \"nobody\""
    );
}

#[test]
fn a_pool_takes_the_first_member_that_can_be_reached() {
    // 第一个成员的 key 没存，第二个的地址解不出，第三个用得了；钉住、轮换都照写的先后。
    let source = "[providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n\
        [providers.lab]\ndriver = \"openai-chat\"\nbase_url = { env = \"LAB_URL\" }\n\n\
        [providers.home]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:11434/v1\"\n\n\
        [pools.mine]\nmodels = [\"deepseek/deepseek-flash\", \"lab/m\", \"home/qwen\", \"deepseek/deepseek-v4-pro\"]\nstrategy = \"rotate\"\n\n\
        [models]\nchat = \"@mine\"\n";
    let held = held();
    let direct =
        direct(&resolved(source), &held.knowledge(), &secrets(&[], &[])).expect("第三个用得了");
    assert_eq!(direct.model, "qwen");
    assert_eq!(direct.base_url, "http://127.0.0.1:11434/v1");

    // key 存上了，第一个就用得了。
    let direct = direct_with(source, &[("deepseek", "sk-one")]);
    assert_eq!(direct.model, "deepseek-flash");
}

fn direct_with(source: &str, stored: &[(&str, &str)]) -> Direct {
    let held = held();
    direct(&resolved(source), &held.knowledge(), &secrets(stored, &[])).expect("用得了")
}

#[test]
fn a_pool_with_nothing_reachable_says_why_the_last_one_failed() {
    let source = "[providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n\
        [pools.mine]\nmodels = [\"deepseek/deepseek-flash\"]\n\n\
        [models]\nchat = \"@mine\"\n";
    let held = held();
    assert_eq!(
        said(direct(
            &resolved(source),
            &held.knowledge(),
            &secrets(&[], &[])
        )),
        "no usable model in pool \"mine\": key secret:deepseek of provider \"deepseek\" is not set"
    );
    // 固定池总在，空着的照解析不出说。
    let resolved = resolved("[models]\nchat = \"@lite\"\n");
    assert_eq!(
        said(direct(&resolved, &held.knowledge(), &secrets(&[], &[]))),
        "pool \"lite\" has no models"
    );
}

/// 照配置的字 `source` 合出来的最终值，`providers.<id>.driver` 的选项接上装着的 `pi/ag`（同核心起来时的做法，施工 MD-1）。
fn resolved_with_installed(source: &str) -> Resolved {
    use miyu_config::Layer;
    use miyu_config::merge::{Layers, merge};
    use miyu_config::parse::parse;
    let items: Vec<miyu_config::Item> = crate::test_support::items()
        .iter()
        .map(|item| miyu_config::driver_ref::with_installed(item, &["pi/ag".to_string()]))
        .collect();
    let parsed = parse(&items, Layer::System, source).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    merge(&items, &layers, &|_| None)
}

/// 一家走扩展驱动的（没写地址、写了取不到的 key）和一家本机的服务。
const WITH_EXTENSION: &str = "[providers.ext]\ndriver = \"pi/ag\"\nkey = { secret = \"ext\" }\n\n\
    [providers.home]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:11434/v1\"\n\n\
    [providers.deepseek]\nkey = { secret = \"deepseek\" }\n\n";

#[test]
fn an_extension_driver_is_handed_back_without_address_or_key() {
    let source = format!("{WITH_EXTENSION}[models]\nchat = \"ext/m\"\n");
    let held = held();
    let direct = direct(
        &resolved_with_installed(&source),
        &held.knowledge(),
        &secrets(&[], &[]),
    )
    .expect("照实交出扩展驱动");
    assert_eq!(direct.driver.as_str(), "pi/ag");
    assert_eq!(direct.base_url, "");
    assert_eq!(direct.key, None);
}

#[test]
fn a_pool_skips_extension_members_for_a_builtin_one() {
    // 先扩展后内置：取内置的那个。
    let source = format!(
        "{WITH_EXTENSION}[pools.mine]\nmodels = [\"ext/m\", \"home/qwen\"]\n\n[models]\nchat = \"@mine\"\n"
    );
    let held = held();
    let builtin = direct(
        &resolved_with_installed(&source),
        &held.knowledge(),
        &secrets(&[], &[]),
    )
    .expect("内置的用得了");
    assert_eq!(builtin.model, "qwen");
    assert_eq!(builtin.driver, Driver::OpenAiChat);

    // 内置的都用不了（key 没存）：交扩展的那个，由调的一方照扩展驱动说。
    let source = format!(
        "{WITH_EXTENSION}[pools.mine]\nmodels = [\"ext/m\", \"deepseek/deepseek-flash\"]\n\n[models]\nchat = \"@mine\"\n"
    );
    let direct = direct(
        &resolved_with_installed(&source),
        &held.knowledge(),
        &secrets(&[], &[]),
    )
    .expect("交扩展的那个");
    assert_eq!(direct.driver.as_str(), "pi/ag");
}
