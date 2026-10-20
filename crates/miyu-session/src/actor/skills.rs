//! 回合开始再找一遍技能（施工 X-11 下，`crate::skills::Watch`）：放开只读的文件夹换成这一刻的，列表变了的交一份新的。

use miyu_config::Values;
use miyu_kernel::facts::Present;
use miyu_kernel::session::Injection;
use miyu_store::resources::SourceError;

use super::Actor;
use crate::TARGET;
use crate::blocking::blocking;
use crate::skills;

impl Actor {
    /// 照会话现在的快照（工具面、预设的编号、system 里那一块）和预设的几层再找一遍，全局开关照这一轮的配置 `values`（施工
    /// X-27）：权限策略、执行器（认用过的技能、沙盒）换上这一刻开着的技能文件夹；列出来的和她最后看到的（`present`）不一样的，交回尾巴上那一份。没交装了的软件包的、
    /// 没有人格刷新那一份的什么都不做；读不了字的照旧，记一行。
    pub(super) async fn skills_turn(
        &mut self,
        present: &[Present],
        values: &Values,
    ) -> Option<Injection> {
        let refresh = self.persona.as_ref()?;
        let watch = refresh.skills.clone()?;
        let resources = refresh.resources.clone();
        let face = refresh.snapshot.tools.clone();
        let pinned = refresh.snapshot.skills.clone();
        let wanted = refresh.snapshot.preset.as_ref().map(|pin| pin.id.clone());
        let presets = refresh
            .presets
            .as_ref()
            .map(|places| places.presets.clone());
        let present = present.to_vec();
        let switches = crate::switches::of(values);
        let looked = blocking(move || {
            let file = wanted
                .zip(presets)
                .and_then(|(id, presets)| presets.find(&id).ok())
                .map(|found| found.file.following(switches));
            let looked = watch.look(&resources, &face, file.as_ref())?;
            let tail = skills::tail(
                &resources,
                looked.listing.as_deref(),
                pinned.as_deref(),
                &present,
            )?;
            Ok::<_, SourceError>((looked.shelves, looked.names, tail))
        })
        .await;
        match looked {
            Ok((shelves, names, tail)) => {
                self.guard.set_shelves(shelves.clone());
                self.tools.set_shelves(shelves, names);
                tail
            }
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "skills not looked at");
                None
            }
        }
    }
}
