//! `/life` 那条路不吃光线程的栈（施工 LF-12 修）：在人生会话里连着看、结束、再开，是核心里调用层数最深的一条路（连接、命令、
//! 结束人生、新开会话）。调试版里每一层的状态先在栈上放一份再挪走，一层套一层；原来这条路在 Linux 上要 1.5 MiB 到 2 MiB 的
//! 栈，Windows 的 CI 上溢出过（线程默认 2 MiB），把整个测试进程带挂。改成深的那几步在定义处就装箱以后，Linux 上 896 KiB 够用。
//!
//! 这里在一条 1.5 MiB 栈的线程上把那一串走一遍：没装箱的时候在 Linux 上就溢出（溢出是整个进程退出，不是一条测试失败）。
//! 留了余量给别的平台帧大一些，不卡得更小：新开会话本身（和 `/life` 无关）在调试版里就要 800 KiB 上下。

/// 线程的栈：原来溢出的大小。
const STACK: usize = 1536 * 1024;

#[test]
fn the_deepest_life_commands_fit_in_a_small_stack() {
    let walked = std::thread::Builder::new()
        .name("life-small-stack".to_string())
        .stack_size(STACK)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("起得了运行时")
                .block_on(crate::life_command::inside_the_life_session());
        })
        .expect("起得了线程")
        .join();
    assert!(walked.is_ok(), "走完了");
}
