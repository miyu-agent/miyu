//! glibc 上真的改得成、还得回去；macOS 上还得回去（施工 LW-18）；Linux 上透明大页关得上（施工 YC-12）；别的平台什么都不做。

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
    // 一堆小块（不走 mmap，留在堆里），放掉以后堆里有一大片空着的。
    let blocks: Vec<Vec<u8>> = (0..32 * 1024)
        .map(|n| vec![(n % 251) as u8; 1024])
        .collect();
    assert_eq!(blocks.len(), 32 * 1024);
    drop(blocks);
    let released = super::trim();
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

#[test]
fn heap_stats_are_there_where_glibc_is() {
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
