//! 会话列表的 `context`（施工 9-5 再补，网页的全部会话页要的「上下文消耗 xx/xxx」）：`used` 是索引记的最近一次主请求的输入
//! （同终端底栏、会话状态的 `used`），`window` 照那一次请求的模型、这时的配置查模型资料；查不到的不写。同一个模型只查一次。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::Listed;

/// 照模型查窗口：`<供应商>/<模型>` 拆开交给 `window`，记下每个模型查到的。
pub(crate) fn fill(listed: &mut [Listed], window: &dyn Fn(&str, &str) -> Option<u64>) {
    let mut known: BTreeMap<String, Option<u64>> = BTreeMap::new();
    for one in listed.iter_mut().filter(|one| one.context.is_some()) {
        let Some(model) = one.context_model.clone() else {
            continue;
        };
        let found = known.entry(model).or_insert_with_key(|model| {
            model
                .split_once('/')
                .and_then(|(provider, name)| window(provider, name))
        });
        one.window = *found;
    }
}

/// 写进一项：知道用了多少的才写，窗口查到的才带。
pub(crate) fn write(item: &mut Value, listed: &Listed) {
    let Some(used) = listed.context else {
        return;
    };
    let mut context = json!({"used": used});
    if let Some(window) = listed.window {
        context["window"] = json!(window);
    }
    item["context"] = context;
}

#[cfg(test)]
mod tests;
