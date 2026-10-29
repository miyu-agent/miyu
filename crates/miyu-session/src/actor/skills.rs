//! 回合开始再找一遍技能（施工 X-11 下，`crate::skills::Watch`）：放开只读的文件夹换成这一刻的，列表变了的交一份新的。

use std::future::Future;
use std::path::PathBuf;

use miyu_config::Values;
use miyu_kernel::facts::Present;
use miyu_kernel::session::Injection;
use miyu_store::resources::SourceError;

use super::Actor;
use crate::TARGET;
use crate::blocking::blocking;
use crate::skills;

/// 阻塞线程里找到的：开着的技能文件夹、认用过的技能的名字、要交的尾巴。
pub(super) type Looked = Result<(Vec<PathBuf>, skills::Names, Option<Injection>), SourceError>;

impl Actor {
    /// 照会话现在的快照（工具面、预设的编号、system 里那一块）和预设的几层再找一遍，全局开关照这一轮的配置 `values`（施工
    /// X-27）：交回在阻塞线程里找的那一件，不借 actor，和刷新人格并着等（施工 PERF-2：原来排在人格后面干等；人格真换了的照新快照再找一次）；
    /// 找回来的交给 [`Actor::skills_take`]。没交装了的软件包的、没有人格刷新那一份的什么都不找。
    pub(super) fn skills_turn(
        &self,
        present: &[Present],
        values: &Values,
    ) -> impl Future<Output = Option<Looked>> + Send + 'static {
        let asking = self.persona.as_ref().and_then(|refresh| {
            let watch = refresh.skills.clone()?;
            let wanted = refresh.snapshot.preset.as_ref().map(|pin| pin.id.clone());
            // 别的目录照这一轮的配置（施工 PT-2 外）：关着的不碰。
            let external = watch.external(values);
            Some((
                watch,
                external,
                refresh.resources.clone(),
                refresh.snapshot.tools.clone(),
                refresh.snapshot.skills.clone(),
                wanted.zip(refresh.presets.clone()),
            ))
        });
        let present = present.to_vec();
        let switches = crate::switches::of(values);
        // 这个人关了的包（施工 U23 末上）：它带的技能下一个回合就不列。
        let off = crate::packages_off::of(values);
        async move {
            let (watch, external, resources, face, pinned, preset) = asking?;
            let looked = blocking(move || {
                let file = preset.and_then(|(id, places)| {
                    let found = places.presets.find(&id).ok()?;
                    Some(found.file.following(switches).switching_off(off))
                });
                let looked = watch.look(&resources, &face, file.as_ref(), &external)?;
                let tail = skills::tail(
                    &resources,
                    looked.listing.as_deref(),
                    pinned.as_deref(),
                    &present,
                )?;
                Ok((looked.shelves, looked.names, tail))
            })
            .await;
            Some(looked)
        }
    }

    /// 找回来的换上：权限策略、执行器（认用过的技能、沙盒）换上这一刻开着的技能文件夹；列出来的和她最后看到的不一样的，交回
    /// 尾巴上那一份。读不了字的照旧，记一行。
    pub(super) fn skills_take(&mut self, looked: Option<Looked>) -> Option<Injection> {
        match looked? {
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
