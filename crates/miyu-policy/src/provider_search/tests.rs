//! 供应商自己的联网工具的判断（设计 72 第二节第 2、3 条，施工 DSR-3）。

use super::{Replaced, SearchOffer, SearchPin, strip};

fn offer() -> SearchOffer {
    SearchOffer {
        setting: "deepseek-responses".to_string(),
        replaces: vec![
            ("web_search".to_string(), "deepseek_web_search".to_string()),
            ("web_fetch".to_string(), "deepseek_read_url".to_string()),
        ],
    }
}

fn face(names: &[&str]) -> Vec<crate::ToolEntry> {
    names
        .iter()
        .map(|name| {
            serde_json::from_value(serde_json::json!({
                "name": name, "description": "A tool.", "parameters": {"type": "object"}, "access": "read"
            }))
            .expect("是一件工具")
        })
        .collect()
}

fn names(face: &[crate::ToolEntry]) -> Vec<&str> {
    face.iter().map(|entry| entry.name.as_str()).collect()
}

const MODEL: &str = "deepseek/deepseek-flash";
const PICKED: &str = "deepseek-responses";

#[test]
fn a_model_that_cannot_offer_it_pins_nothing() {
    let whole = face(&["read", "web_fetch", "web_search"]);
    assert_eq!(SearchPin::judged(None, MODEL, None, PICKED, &whole), None);
    // 原来钉着、激活着的，模型这边给不了了（换了模型、关了思考、改了接口）：不钉了，工具面回到原样。
    let pinned = SearchPin::judged(None, MODEL, Some(&offer()), PICKED, &whole);
    assert_eq!(
        SearchPin::judged(pinned.as_ref(), "openai/gpt", None, PICKED, &whole),
        None
    );
    assert_eq!(names(&strip(whole.clone(), None)), names(&whole));
}

#[test]
fn picking_the_setting_replaces_the_tools_that_are_on_the_face() {
    let whole = face(&["read", "web_fetch", "web_search"]);
    let pin = SearchPin::judged(None, MODEL, Some(&offer()), PICKED, &whole).expect("钉了");
    assert!(pin.active);
    assert_eq!(pin.names(), ["deepseek_web_search", "deepseek_read_url"]);
    assert_eq!(names(&strip(whole, Some(&pin))), ["read"]);
    // 预设关掉了抓网页的：只换搜索那一件。
    let partial = face(&["read", "web_search"]);
    let pin = SearchPin::judged(None, MODEL, Some(&offer()), PICKED, &partial).expect("钉了");
    assert_eq!(
        pin.declared,
        [Replaced {
            tool: "web_search".to_string(),
            with: "deepseek_web_search".to_string()
        }]
    );
    // 两件都关掉的：激活着，可一件都不换、什么都不声明。
    let none = face(&["read"]);
    let pin = SearchPin::judged(None, MODEL, Some(&offer()), PICKED, &none).expect("钉了");
    assert!(pin.active && pin.declared.is_empty());
    assert_eq!(names(&strip(none, Some(&pin))), ["read"]);
}

#[test]
fn another_setting_pins_it_as_not_picked() {
    let whole = face(&["read", "web_fetch", "web_search"]);
    let pin = SearchPin::judged(None, MODEL, Some(&offer()), "auto", &whole).expect("钉了");
    assert!(!pin.active && pin.declared.is_empty());
    assert_eq!(
        names(&strip(whole.clone(), Some(&pin))),
        names(&whole),
        "没选的工具面不动"
    );
}

/// 会话中途只改「联网服务」的不重新判断（设计 72 第二节第 3 条）：引用没变，照原来钉的。
#[test]
fn a_setting_changed_mid_session_does_not_rejudge() {
    let whole = face(&["read", "web_fetch", "web_search"]);
    let on = SearchPin::judged(None, MODEL, Some(&offer()), PICKED, &whole);
    assert_eq!(
        SearchPin::judged(on.as_ref(), MODEL, Some(&offer()), "auto", &whole),
        on,
        "激活着的，改成自动也照旧"
    );
    let off = SearchPin::judged(None, MODEL, Some(&offer()), "auto", &whole);
    assert_eq!(
        SearchPin::judged(off.as_ref(), MODEL, Some(&offer()), PICKED, &whole),
        off,
        "没激活的，中途选上也照旧"
    );
}

/// 换模型时当场重新判断（设计 72 第二节第 3 条）：引用变了，照这一刻的设置。
#[test]
fn switching_the_model_rejudges_with_the_setting_of_the_moment() {
    let whole = face(&["read", "web_fetch", "web_search"]);
    let off = SearchPin::judged(None, MODEL, Some(&offer()), "auto", &whole);
    let other = "deepseek/deepseek-v4-pro";
    let pin = SearchPin::judged(off.as_ref(), other, Some(&offer()), PICKED, &whole).expect("钉了");
    assert_eq!(pin.model, other);
    assert!(pin.active);
    assert_eq!(pin.declared.len(), 2);
}

/// 钉的东西写得进、读得回快照；没换掉一件的不写 `declared`。
#[test]
fn the_pin_round_trips_and_omits_an_empty_list() {
    let whole = face(&["web_fetch", "web_search"]);
    let pin = SearchPin::judged(None, MODEL, Some(&offer()), PICKED, &whole).expect("钉了");
    let text = serde_json::to_string(&pin).expect("写得成");
    assert_eq!(
        text,
        r#"{"model":"deepseek/deepseek-flash","active":true,"declared":[{"tool":"web_search","with":"deepseek_web_search"},{"tool":"web_fetch","with":"deepseek_read_url"}]}"#
    );
    assert_eq!(
        serde_json::from_str::<SearchPin>(&text).expect("读得回"),
        pin
    );
    let off = SearchPin::judged(None, MODEL, Some(&offer()), "auto", &whole).expect("钉了");
    assert_eq!(
        serde_json::to_string(&off).expect("写得成"),
        r#"{"model":"deepseek/deepseek-flash","active":false}"#
    );
}
