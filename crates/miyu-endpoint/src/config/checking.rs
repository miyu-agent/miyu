//! `config.check`：把一段字当成一层的文件查，不生效（施工 8-3；施工 Y-2 下下中上从 `methods.rs` 挪出来，行数门禁）。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Layer;
use miyu_config::merge::{Trust, merge};
use miyu_config::parse::parse;
use miyu_config::problem::{Code, Problem};
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
pub(crate) fn check(core: &Core, peer: Peer, params: CheckParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let problems = check_text(
        &core.config(),
        &words,
        (params.layer.layer(), &params.text),
        None,
    )?;
    Ok(json!({ "problems": problems }))
}

/// 现有的人格、知识库的编号（施工 PC-3）：`check` 照它报写了、却没有的（警告）。拿配置的锁之前列好；列不出来的那一种是空的，
/// 不查（报了全是误报）。
#[derive(Debug, Default)]
pub(crate) struct Known {
    /// 人格的编号。
    pub(crate) personas: Option<std::collections::BTreeSet<String>>,
    /// 知识库的编号。
    pub(crate) knowledge: Option<std::collections::BTreeSet<String>>,
}

/// 把 `text` 当成 `config` 的 `layer` 那一层的文件查，交回给人看的问题（`config.check`；施工 8-30 起 `check` 也用它）。写了
/// `known` 的另报写了、却没有的人格、知识库（施工 PC-3，只有 `check` 交）。
pub(crate) fn check_text(
    config: &Config,
    words: &Human,
    (layer, text): (Layer, &str),
    known: Option<&Known>,
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
    let unknown = known.map_or_else(Vec::new, |known| {
        miyu_config::dangling::unknown_names(
            config.items(),
            &parsed,
            layer,
            &|id| known.personas.as_ref().is_none_or(|ids| ids.contains(id)),
            &|id| known.knowledge.as_ref().is_none_or(|ids| ids.contains(id)),
        )
    });
    let mut found: Vec<&Problem> = parsed
        .problems
        .iter()
        .chain(&missing)
        .chain(&external)
        .chain(&unknown)
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
