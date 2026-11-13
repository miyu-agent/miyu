//! 判法表的每一格（`11-权限与沙盒.md` 第二节，施工 4-3 下）：纯的判法，不碰磁盘。

use miyu_kernel::event::Level;

use super::*;

// 高危的规则只认 bash、zsh 的写法（施工 5-17），Windows 上的 PowerShell 不认。
#[cfg(not(windows))]
mod protected;

#[test]
fn every_cell_of_the_table() {
    let (full, read_only) = (Effective::Full, Effective::ReadOnly);
    let cases = [
        // 数据根：哪一级都拒绝。
        (Zone::Forbidden, full, false, Mark::Forbidden),
        (Zone::Forbidden, full, true, Mark::Forbidden),
        (Zone::Forbidden, read_only, false, Mark::Forbidden),
        // 能读能写的那几片。
        (Zone::Writable, full, true, Mark::Allow),
        (Zone::Writable, read_only, false, Mark::Allow),
        (Zone::Writable, read_only, true, Mark::ReadOnly),
        // 只能读的那几片。
        (Zone::Readable, full, true, Mark::Allow),
        (Zone::Readable, read_only, false, Mark::Allow),
        (Zone::Readable, read_only, true, Mark::ReadOnly),
        // 边界以外：读哪一级都放行（施工 5-4 上）。
        (Zone::Outside, full, false, Mark::Allow),
        (Zone::Outside, full, true, Mark::Allow),
        (Zone::Outside, read_only, false, Mark::Allow),
        (Zone::Outside, read_only, true, Mark::ReadOnly),
        // 别的程序的私密目录：完全放开不问；只读的读问人、写照拒（施工 5-13）。
        (Zone::Private, full, false, Mark::Allow),
        (Zone::Private, full, true, Mark::Allow),
        (Zone::Private, read_only, false, Mark::Ask),
        (Zone::Private, read_only, true, Mark::ReadOnly),
    ];
    for (zone, level, write, expected) in cases {
        assert_eq!(
            mark(level, zone, write),
            expected,
            "{zone:?} {level:?} write={write}"
        );
    }
}

#[test]
fn the_level_in_effect() {
    let permission = |level: Level, read_only: bool| Permission { level, read_only };
    assert_eq!(effective(&permission(Level::Full, false)), Effective::Full);
    assert_eq!(
        effective(&permission(Level::Full, true)),
        Effective::ReadOnly
    );
    // 不认识的级别按最严的算。A16 以前的工作区 `workspace` 也是不认识的了（施工 AUD-4：不再读成完全放开）。
    let legacy: Level = serde_json::from_str(r#""workspace""#).expect("读得出");
    for unknown in [legacy, Level::Other("root".to_string())] {
        assert_eq!(effective(&permission(unknown, false)), Effective::ReadOnly);
    }
}

#[test]
fn a_command_goes_by_whether_the_sandbox_can_be_used() {
    let (full, read_only) = (Effective::Full, Effective::ReadOnly);
    // 沙盒能用：两级都放行，只读在沙盒里跑（施工 5-4 上）。
    for level in [full, read_only] {
        assert_eq!(
            untargeted(level, "shell", Access::Execute, true, &[]),
            Verdict::Allow,
            "{level:?}"
        );
    }
    // 用不了：完全放开照样放行，只读问人，问的时候不提规则。
    assert_eq!(
        untargeted(full, "shell", Access::Execute, false, &[]),
        Verdict::Allow
    );
    let Verdict::Ask {
        module: asker,
        access,
        rule,
        detail,
    } = untargeted(read_only, "shell", Access::Execute, false, &[])
    else {
        panic!("沙盒用不了，只读时执行命令要问人");
    };
    assert_eq!(asker, module());
    assert_eq!(access, Access::Execute);
    assert_eq!(rule, None);
    assert_eq!(
        detail.map(|detail| detail.get().to_string()).as_deref(),
        Some(r#"{"sandbox":false,"tool":"shell"}"#)
    );
}

#[test]
fn a_call_without_paths_goes_by_what_it_does() {
    // 读写不报路径的放行；联网这些，只读问人、完全放开放行，沙盒能用也一样。
    for sandboxed in [false, true] {
        assert_eq!(
            untargeted(Effective::ReadOnly, "x", Access::Read, sandboxed, &[]),
            Verdict::Allow
        );
        assert_eq!(
            untargeted(Effective::ReadOnly, "x", Access::Write, sandboxed, &[]),
            Verdict::Allow
        );
        assert!(matches!(
            untargeted(
                Effective::ReadOnly,
                "fetch",
                Access::Network,
                sandboxed,
                &[]
            ),
            Verdict::Ask { .. }
        ));
        assert_eq!(
            untargeted(Effective::Full, "fetch", Access::Network, sandboxed, &[]),
            Verdict::Allow
        );
    }
}

/// 问人时的说明（施工 D-4）：工具交的几格并进来，键照字母排；工具名由权限策略写，工具交的同名格盖不过它；执行类的写明
/// 这一次不在沙盒里跑，别的不写。
#[test]
fn the_detail_takes_what_the_tool_says() {
    let asking = [
        ("title", "Create a file".to_string()),
        ("command", "touch ~/x".to_string()),
        ("tool", "not me".to_string()),
    ];
    assert_eq!(
        described("shell", &Access::Execute, &asking).get(),
        r#"{"command":"touch ~/x","sandbox":false,"title":"Create a file","tool":"shell"}"#
    );
    assert_eq!(
        described("fetch", &Access::Network, &[]).get(),
        r#"{"tool":"fetch"}"#
    );
    // 沙盒用不了时每条命令都问：说明照样带标题和命令（D-1 补）。
    let Verdict::Ask { detail, .. } = untargeted(
        Effective::ReadOnly,
        "shell",
        Access::Execute,
        false,
        &asking[..2],
    ) else {
        panic!("沙盒用不了，要问人");
    };
    assert_eq!(
        detail.map(|detail| detail.get().to_string()).as_deref(),
        Some(r#"{"command":"touch ~/x","sandbox":false,"title":"Create a file","tool":"shell"}"#)
    );
}

/// 在场所里做的事（`venue`，施工 O-31 前）：场所会话里放行，不问；本机的会话拒绝，写给她出厂的那一句，说法是
/// `core/permissions/not-in-venue`。
#[test]
fn a_venue_action_runs_only_in_a_venue() {
    assert_eq!(in_venue(&Place::Venue), Verdict::Allow);
    let local = Place::local("Not here.\n".to_string());
    assert_eq!(
        in_venue(&local),
        Verdict::Deny {
            module: module(),
            text: "Not here.\n".to_string(),
            human: Some(miyu_kernel::event::Said::new(
                "core/permissions/not-in-venue"
            )),
        }
    );
}

/// 高危的（施工 5-17）：路径已经判拒的照拒，不被改成问人；灭顶的拒，拿的是它那一句；别的问人，不提规则。
#[test]
fn a_hazard_never_softens_a_refusal() {
    let worded = |text: &str| Worded {
        text: text.to_string(),
        said: None,
    };
    let asks = || {
        miyu_tool::Hazard::Asks(vec![miyu_tool::HazardAsk {
            code: "git_force_push",
            class: Some("git"),
        }])
    };
    let shell = ("shell", Access::Execute);
    let refused = deny(worded("forbidden"));
    assert_eq!(
        hazardous(&refused, shell.clone(), asks(), &[], &[]),
        Some(refused.clone())
    );
    // 路径拒了的，这一类关掉了也照拒。
    assert_eq!(
        hazardous(&refused, shell.clone(), asks(), &["git"], &[]),
        Some(refused)
    );
    assert_eq!(
        hazardous(
            &Verdict::Allow,
            shell.clone(),
            miyu_tool::Hazard::Refused(worded("never")),
            &[],
            &[]
        ),
        Some(deny(worded("never")))
    );
    let Some(Verdict::Ask { rule, detail, .. }) =
        hazardous(&Verdict::Allow, shell, asks(), &[], &[])
    else {
        panic!("该问人");
    };
    assert_eq!(rule, None);
    assert!(
        detail
            .expect("有说明")
            .get()
            .contains(r#""hazard":"git_force_push""#)
    );
}
