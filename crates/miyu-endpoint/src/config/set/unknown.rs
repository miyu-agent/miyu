//! `config.set` 写成了以后，改了的几项里选人格、选知识库、选预设的指着现在没有的（施工 CK-1）：照 `check` 的同一条警告
//! （`unknown_persona`、`unknown_knowledge`、`unknown_preset`，`dangling.rs` 的 `unknown_names`）放进回应的 `problems`，
//! 照连接的语言说。照收、不挡：人格、预设可能以后才建，同软件包设置项的这一种（施工 PC-3）。一条都没有的不带这一格。

use serde_json::{Value, json};

use miyu_config::{Kind, Layer};

use crate::Core;
use crate::check::known::known;
use crate::config::methods::{said, words};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 写成了的回应 `reply`（`keys` 是改了的几项）里补上指着没有的那几条警告。
pub(crate) async fn warned(core: &Core, peer: Peer, mut reply: Value) -> Result<Value, Refusal> {
    let Some(changed) = reply["keys"].as_object().filter(|keys| !keys.is_empty()) else {
        return Ok(reply);
    };
    let changed: Vec<String> = changed.keys().cloned().collect();
    let known = known(core).await;
    let words = words(core, peer.language)?;
    let config = core.config();
    let layers = config.layers(None);
    let parsed = &config.file().parsed;
    let found =
        miyu_config::dangling::unknown_names(config.items(), parsed, Layer::System, &|kind, id| {
            let ids = match kind {
                Kind::Persona => &known.personas,
                Kind::Knowledge => &known.knowledge,
                _ => &known.presets,
            };
            ids.as_ref().is_none_or(|ids| ids.contains(id))
        });
    let mut problems = Vec::new();
    for problem in found.iter().filter(|problem| {
        problem.key.as_deref().is_some_and(|key| {
            changed
                .iter()
                .any(|one| key == one || key.starts_with(&format!("{one}[")))
        })
    }) {
        problems.push(said(
            &config,
            &layers,
            problem,
            Some(config.file()),
            &words,
        )?);
    }
    if !problems.is_empty() {
        reply["problems"] = json!(problems);
    }
    Ok(reply)
}
