//! 会话列表的 `context`（施工 9-5 再补）：照模型查窗口、同一个模型只查一次；知道用了多少的才写，窗口查到的才带。

use std::cell::Cell;

use super::*;

fn listed(context: Option<u64>, model: Option<&str>) -> Listed {
    Listed {
        id: miyu_kernel::id::SessionId::parse("01a15340-b949-7220-aeb1-30f2899eb504")
            .expect("合写法"),
        oneshot: false,
        parent: None,
        title: String::new(),
        named: false,
        pinned: false,
        cwd: "~".to_string(),
        busy: false,
        last_active: miyu_kernel::time::Timestamp::from_unix_millis(0).expect("是时刻"),
        preview: String::new(),
        venue: "local".to_string(),
        persona: None,
        preset: None,
        project: None,
        tokens: 0,
        context,
        context_model: model.map(str::to_string),
        window: None,
        default_workspace: None,
        dream: None,
        origin: None,
    }
}

#[test]
fn windows_are_looked_up_once_per_model() {
    let asked = Cell::new(0);
    let window = |provider: &str, model: &str| {
        asked.set(asked.get() + 1);
        (provider == "deepseek" && model == "deepseek-flash").then_some(1_000_000)
    };
    let mut all = [
        listed(Some(100), Some("deepseek/deepseek-flash")),
        listed(Some(200), Some("deepseek/deepseek-flash")),
        listed(Some(300), Some("other/model")),
        listed(None, Some("deepseek/deepseek-flash")),
    ];
    fill(&mut all, &window);
    assert_eq!(asked.get(), 2, "同一个模型只查一次，没用量的不查");
    let mut item = json!({});
    write(&mut item, &all[0]);
    assert_eq!(item["context"], json!({"used": 100, "window": 1_000_000}));
    let mut item = json!({});
    write(&mut item, &all[2]);
    assert_eq!(item["context"], json!({"used": 300}), "查不到窗口的不写");
    let mut item = json!({});
    write(&mut item, &all[3]);
    assert!(item.get("context").is_none(), "不知道用了多少的不写");
}
