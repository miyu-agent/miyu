//! 拆掉的配置项（施工 PV-RM：私密目录那三项 `sandbox.private.enabled`、`.add`、`.remove`）：核心登记的清单里没有了，老配置里
//! 还写着的照不在清单里的键报 `unknown_key`，只是警告，不进最终值，别的项照常读（同施工 AUD-4 的 `permission.level`、LV-5 的
//! `sandbox.*`）。

use miyu_config::Layer;
use miyu_config::parse::parse;
use miyu_config::problem::{Code, Severity};
use miyu_core::settings::items;

#[test]
fn the_private_dirs_keys_only_warn_as_unknown() {
    let source = "[permission]\nstart_read_only = true\n\n[sandbox.private]\nenabled = false\nadd = [\"~/notes\"]\nremove = [\"~/.ssh\"]\n";
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert_eq!(
        parsed
            .entries
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["permission.start_read_only"],
        "只有还在的那一项进最终值"
    );
    let found: Vec<_> = parsed
        .problems
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.code.severity(),
                problem.key.as_deref(),
            )
        })
        .collect();
    let warned = |key| (Code::UnknownKey, Severity::Warning, Some(key));
    assert_eq!(
        found,
        [
            warned("sandbox.private.enabled"),
            warned("sandbox.private.add"),
            warned("sandbox.private.remove"),
        ]
    );
}
