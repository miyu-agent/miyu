//! glibc 上真的改得成、还得回去，线程分配区的顶上不多留（施工 LW-25）；macOS 上还得回去（施工 LW-18）；Linux 上透明大页关得上（施工 YC-12）；别的平台什么都不做。

#[test]
fn the_allocator_can_be_tuned_where_glibc_is() {
    assert_eq!(
        super::tune(),
        cfg!(all(target_os = "linux", target_env = "gnu"))
    );
}

#[test]
fn huge_pages_are_off_after_tuning_on_linux() {
    super::tune();
    assert_eq!(super::huge_pages_off(), cfg!(target_os = "linux"));
}

/// 放掉一大片、再看还没还的两条测试一条一条跑：测试是几条线程一起跑的，一条叫分配器还的时候会把另一条刚放掉的也还了，
/// 另一条正在分的又会把占用顶上去，各自看到的就不作数。
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn freed_small_blocks_are_given_back_where_the_allocator_keeps_them() {
    let _alone = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    super::tune();
    // 一堆小块（不走 mmap，留在堆里），放掉以后堆里有一大片空着的。上面再压一块留着：不然放掉的并进顶上那一块，调好以后
    // glibc 放的时候当场就还了（施工 LW-25「顶上不多留」），再叫它还就没有东西可还；压住以后放掉的只能等 `trim` 来还。
    let blocks: Vec<Vec<u8>> = (0..32 * 1024)
        .map(|n| vec![(n % 251) as u8; 1024])
        .collect();
    let fence = vec![1_u8; 1024];
    assert_eq!(blocks.len(), 32 * 1024);
    drop(blocks);
    let released = super::trim();
    drop(fence);
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        assert!(released, "glibc 上空着的还得回去");
    } else if cfg!(target_os = "macos") {
        // macOS 上这个大小的块有时放掉当场就还了，再叫它还就没有东西可还：这里只查调得了；真还了多少看下一条。
    } else {
        assert!(!released);
    }
}

/// macOS 上还回去的真的不再算在这个进程头上（施工 LW-18）：32 MB 的小块写过、放掉，叫分配器还一次，进程的占用
/// （`phys_footprint`）要落回去一半以上。块取 512 字节：这个大小放掉以后分配器自己不还（实测 1 KiB 的块有时放掉当场就
/// 还了，再叫它还就看不出差别）。
#[cfg(target_os = "macos")]
#[test]
fn what_is_given_back_leaves_the_footprint_on_macos() {
    let _alone = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let footprint = || {
        crate::usage::usage()
            .and_then(|usage| usage.footprint)
            .expect("macOS 上读得到自己的占用")
    };
    let blocks: Vec<Vec<u8>> = (0..64 * 1024).map(|n| vec![(n % 251) as u8; 512]).collect();
    let held = footprint();
    drop(blocks);
    let freed = footprint();
    let released = super::trim();
    let after = footprint();
    assert!(
        held.saturating_sub(after) >= 16 * 1024 * 1024,
        "放掉 32 MB、还了一次以后占用从 {held} 到 {after}（刚放掉时 {freed}，trim 交回 {released}）"
    );
}

/// 顶上不多留（施工 LW-25）：调好以后，一条新线程在它的分配区里写过 4 MB 的中块（不走 mmap）、全部放掉，放掉的并进
/// 顶上那一块，当场还给系统，各个 arena 从系统要来的合计回到原来的样子，差不到 64 KiB。glibc 默认顶上留 128 KiB，
/// 这一条在没调过的进程里过不了。
///
/// 量的是整个进程各个 arena 的合计，所以另起一个只跑这一条的进程来量（施工 LW-31）：这个测试程序里别的测试在别的线程上
/// 同时分、同时放，它们的 arena 涨了、缩了都算进合计里，同一个进程里量会偶尔差出几百 KiB 到几 MB。
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn a_thread_arena_gives_its_top_back_after_tuning() {
    if std::env::var_os(ALONE).is_none() {
        let exe = std::env::current_exe().expect("找得到测试程序自己");
        let out = std::process::Command::new(exe)
            .args([
                "--exact",
                "tests::a_thread_arena_gives_its_top_back_after_tuning",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(ALONE, "1")
            .output()
            .expect("起得来");
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && said.contains("1 passed"),
            "{said}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    assert!(super::tune());
    let arenas = || super::stats().expect("glibc 上有").arenas;
    // 先让一条线程把分配区建好再量：头一次建分配区要来的那一截不算在这里面。
    std::thread::spawn(|| drop(vec![1_u8; 1024]))
        .join()
        .expect("线程跑得完");
    let before = arenas();
    let held = std::thread::spawn(move || {
        let blocks: Vec<Vec<u8>> = (0..40).map(|n| vec![(n % 251) as u8; 100 * 1024]).collect();
        let held = arenas();
        drop(blocks);
        held
    })
    .join()
    .expect("线程跑得完");
    let after = arenas();
    assert!(
        held >= before + 3 * 1024 * 1024,
        "写的时候要来了：{before} → {held}"
    );
    assert!(
        after < before + 64 * 1024,
        "放掉以后还回去：写之前 {before}，写的时候 {held}，放掉以后 {after}"
    );
}

/// [`a_thread_arena_gives_its_top_back_after_tuning`] 另起的那个进程带着的记号：带着的才真的量。
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const ALONE: &str = "MIYU_HEAP_TEST_ALONE";

#[test]
fn heap_stats_are_there_where_glibc_is() {
    let _alone = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let kept: Vec<Vec<u8>> = (0..1024).map(|n| vec![(n % 251) as u8; 1024]).collect();
    let stats = super::stats();
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        let stats = stats.expect("glibc 上有");
        assert!(stats.used >= 1024 * 1024, "分出去的一兆在用：{stats:?}");
        assert!(stats.arenas >= stats.used, "{stats:?}");
    } else {
        assert_eq!(stats, None);
    }
    assert_eq!(kept.len(), 1024);
}
