//! 拆掉的配置项（施工 PV-RM：私密目录那三项 `sandbox.private.enabled`、`.add`、`.remove`；施工 UX-1：关包的
//! `packages.<编号>.off`）：核心登记的清单里没有了，老配置里还写着的照不在清单里的键报 `unknown_key`，只是警告，不进最终值，
//! 别的项照常读（同施工 AUD-4 的 `permission.level`、LV-5 的 `sandbox.*`）。

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

/// 施工 UX-1：关包的那一项删了，包的开关只有「启用」（`package.enable`、`package.disable`）。「启用」也不写在配置文件里：
/// 写了的一样照不认识的键。
#[test]
fn the_package_off_key_only_warns_as_unknown() {
    let source = "[permission]\nstart_read_only = true\n\n[packages.kit]\noff = false\n\n[packages.relay]\nenabled = true\n";
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert_eq!(
        parsed
            .entries
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["permission.start_read_only"]
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
    assert_eq!(
        found,
        [
            (
                Code::UnknownKey,
                Severity::Warning,
                Some("packages.kit.off")
            ),
            (
                Code::UnknownKey,
                Severity::Warning,
                Some("packages.relay.enabled")
            ),
        ]
    );
}
