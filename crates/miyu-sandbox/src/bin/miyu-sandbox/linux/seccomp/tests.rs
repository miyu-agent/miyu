//! 过滤的跳法（施工 MB-1）：每一条跳的目标都落在过滤里面，交回的只有放行和 `EACCES` 两种。真装上以后的效果在
//! `tests/linux.rs` 里经助手量。

use super::*;

#[test]
fn every_jump_lands_inside_and_only_two_answers_come_back() {
    let filter = program();
    for (at, op) in filter.iter().enumerate() {
        if op.code == JEQ || op.code == JGE {
            for skip in [op.jt, op.jf] {
                assert!(
                    at + 1 + usize::from(skip) < filter.len(),
                    "第 {at} 条跳出去了"
                );
            }
        }
    }
    let answers: Vec<u32> = filter
        .iter()
        .filter(|op| op.code == RET)
        .map(|op| op.k)
        .collect();
    assert_eq!(answers, [RET_ALLOW, RET_ERRNO | EACCES]);
    assert_eq!(filter.last().map(|op| op.code), Some(RET), "最后一条交回");
}

/// 照内核的规矩跑一遍过滤：交回 `(架构, 系统调用号, 第一个参数)` 这一次调用得到的答复。只认过滤里用到的几种指令。
fn answer(filter: &[libc::sock_filter], arch: u32, nr: u32, arg0: u32) -> u32 {
    let mut acc = 0;
    let mut at = 0;
    loop {
        let op = filter[at];
        match op.code {
            LD_ABS => {
                acc = match op.k {
                    NR => nr,
                    ARCH => arch,
                    ARG0 => arg0,
                    other => panic!("第 {at} 条读了认不出的位置 {other}"),
                };
                at += 1;
            }
            JEQ | JGE => {
                let taken = if op.code == JEQ {
                    acc == op.k
                } else {
                    acc >= op.k
                };
                at += 1 + usize::from(if taken { op.jt } else { op.jf });
            }
            RET => return op.k,
            other => panic!("第 {at} 条是认不出的指令 {other:#x}"),
        }
    }
}

/// `io_uring_setup` 的号：x86_64、aarch64 和它们的 32 位都是 425（5.1 起新加的系统调用各架构同号）。
const IO_URING_SETUP_NR: u32 = 425;

/// 审计 #015（施工 AU-P1）：`io_uring` 的 `IORING_OP_SOCKET`（5.19 起）不经 `socket` 系统调用就能新建 AF_UNIX 套接字，过滤
/// 只看系统调用号，看不到它。整个不准建 io_uring（同 Docker 的默认规则）：这个架构的、32 位的都回 `EACCES`。原来的几条照旧。
#[test]
fn io_uring_cannot_be_set_up_and_the_old_answers_stay() {
    let filter = program();
    let deny = RET_ERRNO | EACCES;
    let ask = |arch, nr, arg0| answer(&filter, arch, nr, arg0);
    assert_eq!(
        ask(NATIVE.0, IO_URING_SETUP_NR, 1),
        deny,
        "这个架构的 io_uring_setup"
    );
    assert_eq!(
        ask(COMPAT.0, IO_URING_SETUP_NR, 1),
        deny,
        "32 位的 io_uring_setup"
    );
    assert_eq!(ask(NATIVE.0, NATIVE.1, AF_UNIX), deny, "socket(AF_UNIX)");
    assert_eq!(ask(NATIVE.0, NATIVE.1, 2), RET_ALLOW, "socket(AF_INET)");
    assert_eq!(
        ask(COMPAT.0, COMPAT.2, AF_UNIX),
        deny,
        "32 位的 socket(AF_UNIX)"
    );
    assert_eq!(
        ask(COMPAT.0, COMPAT.2, 2),
        RET_ALLOW,
        "32 位的 socket(AF_INET)"
    );
    assert_eq!(ask(COMPAT.0, COMPAT.1, 0), deny, "socketcall");
    assert_eq!(ask(NATIVE.0, X32 | NATIVE.1, 2), deny, "x32");
    assert_eq!(ask(0x1234, 0, 0), deny, "认不出的架构");
    assert_eq!(ask(NATIVE.0, 0, 0), RET_ALLOW, "别的系统调用");
    assert_eq!(ask(COMPAT.0, 3, 0), RET_ALLOW, "32 位别的系统调用");
}
