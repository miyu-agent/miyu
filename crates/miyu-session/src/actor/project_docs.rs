//! 回合开始找一遍项目指令（施工 P-14，`crate::project_docs`）：没交过的、变了的交一块。本机的会话、预设开着的才找。

use std::future::Future;
use std::path::PathBuf;

use miyu_kernel::facts::Present;
use miyu_kernel::session::Injection;
use miyu_store::resources::ResourceRoot;

use super::Actor;
use crate::TARGET;
use crate::agents::LOCAL;
use crate::blocking::blocking;
use crate::project_docs::{self, MODULE};

impl Actor {
    /// 照会话现在的工作目录找一遍，和她这一段上下文里看到的（`present`）比。场所会话、预设关了的、认不出工作目录的不找；读
    /// 不了字的记一行，这一轮不交。要的先拿出来，等的时候不借着 actor（它不是 `Sync`）。
    pub(super) fn project_docs_turn(
        &self,
        present: &[Present],
    ) -> impl Future<Output = Option<Injection>> + Send + 'static {
        let asking = self
            .project_docs_ask()
            .map(|(cwd, resources)| (cwd, resources, present.to_vec()));
        async move {
            let (cwd, resources, present) = asking?;
            let tail = blocking(move || {
                let texts = resources
                    .project_doc_texts()
                    .map_err(|error| error.to_string())?;
                project_docs::check(&texts)?;
                let found = project_docs::find(&cwd);
                Ok::<_, String>(project_docs::tail(&texts, found.as_ref(), &present))
            })
            .await;
            tail.unwrap_or_else(|error| {
                tracing::warn!(target: TARGET, error = %error, "project instructions not looked at");
                None
            })
        }
    }

    /// 这一轮要不要找、照哪个目录找：本机的会话、预设没关的、认得出工作目录的。
    fn project_docs_ask(&self) -> Option<(PathBuf, ResourceRoot)> {
        let refresh = self.persona.as_ref()?;
        let off = refresh
            .snapshot
            .preset
            .as_ref()
            .is_some_and(|pin| pin.off.iter().any(|feature| feature == MODULE));
        if refresh.venue.as_str() != LOCAL || off {
            return None;
        }
        let cwd = real(self.session.cwd(), self.tools.home())?;
        Some((cwd, refresh.resources.clone()))
    }
}

/// 头报上来的工作目录 `cwd`（`~/src/miyu` 这样）换成真实的路径：`~` 开头的照家目录 `home` 接；不是绝对路径的认不出。
fn real(cwd: &str, home: Option<&std::path::Path>) -> Option<PathBuf> {
    let path = match miyu_fs::tilde(cwd) {
        Some(rest) => home?.join(rest),
        None => PathBuf::from(cwd),
    };
    path.is_absolute().then_some(path)
}
