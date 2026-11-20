//! 快照里的预设现在是什么样（施工 P-2 下，从 `persona.rs` 挪出来：那边到了 500 行）：回合开始照快照里预设的编号重新找一遍。

use miyu_config::Values;
use miyu_policy::preset::{Chosen, MEMORY};

use super::Refresh;

/// 快照里的预设现在是什么样（施工 P-2 下；O-2 中重新筛工具面也要它）。
pub(super) enum Preset {
    /// 这个会话没有预设：以前造的、测试里造的，不筛。
    Nothing,
    /// 明着选了无工具（施工 P-16 下）：工具面是空的。`changed` 是这一轮刚换成它的。
    NoTools { changed: bool },
    /// 找不回来：以前造的快照没有指纹，没交预设的几层。
    Unknown,
    /// 没改：现在找到的那一份。
    Same(Chosen),
    /// 改了：新找到的（记忆照上一份快照开没开、这个预设开没开，施工 FX-8）。
    Changed(Chosen),
}

/// 照快照里预设的编号重新找一遍（施工 P-2 下），全局开关照这一轮的配置 `values`（施工 X-27）。
pub(super) fn preset(refresh: &Refresh, values: &Values) -> Result<Preset, String> {
    if refresh.snapshot.no_tools {
        return Ok(Preset::NoTools { changed: false });
    }
    let Some(pin) = &refresh.snapshot.preset else {
        return Ok(Preset::Nothing);
    };
    let Some(places) = &refresh.presets else {
        return Ok(Preset::Unknown);
    };
    if pin.digest.is_none() {
        return Ok(Preset::Unknown);
    }
    let found = places
        .presets
        .find(&pin.id)
        .map_err(|error| error.to_string())?;
    let features = &places.features();
    // 上一份快照里记忆开着的，照预设文件开不开（施工 FX-8）；关着的照旧关着。照快照里的预设、不照范围：无人格的会话范围是 `off`、
    // 预设里记忆却记着开，照范围算会让它们升级以后白换一次快照、断一次缓存。代价：预设文件把记忆关了又打开的，这个会话要换一次
    // 预设（`recast.rs` 照开会话时的范围）才回来。
    let memory = !pin.off.iter().any(|feature| feature == MEMORY);
    // 关着的包（施工 UX-1 补：开关在 `extensions.json`）照这一轮的开关扣下：开着的会话下一个回合就没有它的东西。
    let file = found
        .file
        .following(crate::switches::of(values))
        .switching_off(places.switched_off());
    let chosen = Chosen::new(found.id, file, features).keeping_memory(memory, features);
    let chosen = chosen.situated(features, refresh.images, refresh.venue.as_str());
    // 以前的快照记的是包的编号（施工 F-3 上）：照现在的功能读一样的算没改，不为改了写法换一次快照。
    Ok(match pin.means_the_same(&chosen.pin(), features) {
        true => Preset::Same(chosen),
        false => Preset::Changed(chosen),
    })
}
