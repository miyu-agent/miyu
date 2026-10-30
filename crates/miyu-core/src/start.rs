//! 起来的后半段，在运行时里（施工 Y-12 中上从 `main` 挪出来）：开套接字，建工作区、读清单和配置、换运行日志的级别、生成参考文件，接着 [`super::run`]。

use std::process::ExitCode;

use miyu_ipc::{Dirs, Lock};
use miyu_store::env::Env;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use super::{Live, Options, failed, load_packages, models, run, settings};

/// 后半段：见这一页开头。`logging` 是起来时照 `MIYU_LOG` 定的级别和装好的运行日志，读完配置照 `log.level` 换。
pub(super) async fn start(
    env: Env,
    (root, resources, lock): (DataRoot, ResourceRoot, Lock),
    options: Options,
    (level, log): (&miyu_log::Level, &miyu_log::Guard),
    catalog: models::Early,
) -> ExitCode {
    let opened = match miyu_ipc::open_locked(&root, &Dirs::current(), lock) {
        Ok(opened) => opened,
        Err(error) => return failed("socket", error.to_string()),
    };
    if let Err(error) = root.prepare_workspace() {
        return failed("home", error.to_string());
    }
    crate::leftovers::tidy(&root);
    let mut found = load_packages(&resources, &root);
    let packaged = settings::Packaged::of(&mut found);
    let config = settings::read(&root, env.home.as_deref(), &packaged);
    settings::log_level(&config, level, log);
    let locale = miyu_store::env::locale();
    settings::generate(
        &root,
        &resources,
        locale.as_deref(),
        &config.resolved().values(),
        config.items(),
        &packaged.manifests,
    );
    let live = Live {
        levels: log.levels(),
        locale,
        found,
    };
    run(
        env,
        (root, resources, opened),
        options,
        (config, live),
        catalog,
    )
    .await
}
