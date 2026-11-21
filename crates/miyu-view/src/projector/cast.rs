//! 人格、预设、权限级别（`internal/blueprint/view.md`「条目」的 `reply`、「`notice` 的 `what`」的 `persona`）：照 `session.created`
//! 起头，人换的人格、预设先记着，下一回换上快照（带 `policy` 的那一条）才算数、出一条旁白，同内核、会话那一层（施工 9-8 十三补、
//! 十四补）；钉着的模型没了、退回默认的另起一条旁白。
//!
//! 翻页切在开头以后的（施工 FX-16）：切点前的这几条照 [`Projector::feed_page`] 照序号插着学，学的和喂的走同一套记法，只是不出
//! 旁白，所以这一页里回复的人格、换上时的旁白、消息的权限级别和整份喂的一样。

use miyu_kernel::event::{Body as EventBody, Event, PolicyChanged, SessionCreated};

use super::Projector;
use super::users::level;
use crate::entry::{EntryId, ModelSwap, Recast, Swapped, Was};
use crate::notice::Notice;

impl Projector {
    /// 会话建好了：权限级别、人格、预设照它起头。
    pub(super) fn created(&mut self, created: &SessionCreated) {
        self.level = Some(level(&created.permission));
        self.persona = (created.persona.clone(), None);
        self.preset = (created.preset.clone(), None);
    }

    /// 换了策略：记下权限级别；钉着的模型没了、退回默认的，另起一条。
    pub(super) fn policy(&mut self, event: &Event, changed: &PolicyChanged) {
        self.choose(changed);
        if changed.policy.is_some()
            && let Some(recast) = self.take_up()
        {
            self.notice(EntryId::event(event.seq), event.at, Notice::Persona(recast));
        }
        let (Some(replaced), Some(model)) = (&changed.replaced, &changed.model) else {
            return;
        };
        self.notice(
            EntryId::event(event.seq),
            event.at,
            Notice::Model(ModelSwap {
                endpoint: None,
                model: model.clone(),
                from: Some(Was {
                    endpoint: None,
                    model: replaced.clone(),
                }),
                why: "replaced".to_string(),
                class: None,
            }),
        );
    }

    /// 喂翻出来的一页（施工 FX-16）：先照 `earlier`（切点前的日志，照序号）学派出去的任务（[`Projector::learn`]），再把这一页的
    /// 事件 `page` 照先后喂一遍。切点前定人格、预设、权限级别的那几条照序号插着学：这一页最前面带进来的触发消息比切点早，它
    /// 那一刻的权限级别照它以前的算，不照切点的（同整份喂的）。学的不出旁白。
    pub fn feed_page(&mut self, earlier: &[Event], page: &[&Event]) {
        self.learn(earlier);
        let mut cast = earlier
            .iter()
            .filter(|event| {
                matches!(
                    event.body,
                    EventBody::SessionCreated(_) | EventBody::PolicyChanged(_)
                )
            })
            .peekable();
        for event in page {
            while let Some(earlier) = cast.next_if(|earlier| earlier.seq < event.seq) {
                self.learn_cast(earlier);
            }
            self.event(event);
        }
    }

    /// 切点前的一条：只记人格、预设、权限级别，记着还没换上的也算，不出旁白。
    fn learn_cast(&mut self, earlier: &Event) {
        match &earlier.body {
            EventBody::SessionCreated(created) => self.created(created),
            EventBody::PolicyChanged(changed) => {
                self.choose(changed);
                if changed.policy.is_some() {
                    self.take_up();
                }
            }
            _ => {}
        }
    }

    /// 记下换了的权限级别；人换的人格、预设先记着（施工 9-8 十三补）。
    fn choose(&mut self, changed: &PolicyChanged) {
        if let Some(permission) = &changed.permission {
            self.level = Some(level(permission));
        }
        if let Some(persona) = &changed.persona {
            self.persona.1 = Some(Some(persona.clone()).filter(|persona| !persona.is_empty()));
        }
        if let Some(preset) = &changed.preset {
            self.preset.1 = Some(preset.clone());
        }
    }

    /// 快照换上了：记着的人格、预设算数，交回真变了的那几样；都没变的没有。
    fn take_up(&mut self) -> Option<Recast> {
        let persona = self.persona.1.take().and_then(|to| {
            let from = std::mem::replace(&mut self.persona.0, to.clone());
            (from != to).then_some(Swapped { from, to })
        });
        let preset = self.preset.1.take().and_then(|to| {
            let from = self.preset.0.replace(to.clone());
            (from.as_ref() != Some(&to)).then_some(Swapped { from, to: Some(to) })
        });
        (persona.is_some() || preset.is_some()).then_some(Recast { persona, preset })
    }
}
