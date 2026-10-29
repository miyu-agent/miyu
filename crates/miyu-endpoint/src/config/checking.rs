//! `config.check`：把一段字当成一层的文件查，不生效（施工 8-3；施工 Y-2 下下中上从 `methods.rs` 挪出来，行数门禁）。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Layer;
use miyu_config::merge::{Trust, merge};
use miyu_config::parse::parse;
use miyu_config::problem::{Code, Problem};
use miyu_kernel::id::AccountId;
use miyu_store::human::Human;

use super::Config;
use super::methods::{said, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `config.check` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct CheckParams {
    /// 当成哪一层的文件查。
    layer: LayerParam,
    /// 要查的字。
    text: String,
}

/// 能查的两层。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LayerParam {
    System,
    Project,
}

impl LayerParam {
    fn layer(self) -> Layer {
        match self {
            LayerParam::System => Layer::System,
            LayerParam::Project => Layer::Project,
        }
    }
}

/// `config.check`：把 `text` 当成一层的文件查，不生效。项目配置照「收紧」和另外几层合出来的比，不看信没信任。
pub(crate) fn check(
    core: &Core,
    (account, peer): (&AccountId, Peer),
    params: CheckParams,
) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let mine = core.mine(account);
    let problems = check_text(&mine.config(), &words, params.layer.layer(), &params.text)?;
    Ok(json!({ "problems": problems }))
}

/// 把 `text` 当成 `config` 的 `layer` 那一层的文件查，交回给人看的问题（`config.check`；施工 8-30 起 `check` 也用它）。
pub(crate) fn check_text(
    config: &Config,
    words: &Human,
    layer: Layer,
    text: &str,
) -> Result<Vec<Value>, Refusal> {
    let parsed = match parse(config.items(), layer, text) {
        Ok(parsed) => parsed,
        Err(problem) => {
            let layers = config.layers(None);
            return Ok(vec![said(config, &layers, &problem, None, words)?]);
        }
    };
    let mut layers = config.layers(None);
    let missing = config.missing_if(&parsed, layer);
    let external = super::private::external_dirs(&parsed, layer);
    let mut found: Vec<&Problem> = parsed
        .problems
        .iter()
        .chain(&missing)
        .chain(&external)
        .collect();
    let tightening;
    if layer == Layer::Project {
        layers.project = Some((&parsed, Trust::Trusted));
        let merged = merge(config.items(), &layers, &|_| None);
        tightening = merged
            .problems
            .into_iter()
            .filter(|problem| problem.code == Code::NotTightening)
            .collect::<Vec<_>>();
        found.extend(tightening.iter());
    }
    let mut problems = Vec::new();
    for problem in found {
        problems.push(said(config, &layers, problem, None, words)?);
    }
    Ok(problems)
}
