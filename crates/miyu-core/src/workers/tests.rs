//! 工作线程数：默认照核数并夹住，配置写的照它（施工 CPU-1）。

use super::*;

/// 核数是 `n` 的机器。
fn cores(n: usize) -> Option<NonZeroUsize> {
    NonZeroUsize::new(n)
}

#[test]
fn by_default_it_follows_the_cores_within_bounds() {
    assert_eq!(pick(0, cores(12)), 12);
    assert_eq!(pick(0, cores(1)), FEWEST, "一个核的也开两个");
    assert_eq!(pick(0, cores(256)), MOST, "核再多也只开 64 个");
    assert_eq!(pick(0, None), FEWEST, "取不到核数的照 2");
}

#[test]
fn what_is_written_wins_and_is_clamped() {
    assert_eq!(pick(1, cores(12)), FEWEST, "写 1 的照 2");
    assert_eq!(pick(3, cores(12)), 3);
    assert_eq!(pick(64, cores(12)), 64);
    assert_eq!(pick(3, None), 3);
}

#[test]
fn the_config_text_is_read_for_this_one_key() {
    assert_eq!(written("[core]\nworkers = 0\n"), 0);
    assert_eq!(written("[core]\nworkers = 1\n"), 1);
    assert_eq!(written("[core]\nworkers = 3\n"), 3);
    assert_eq!(written("core.workers = 64\n"), 64);
    assert_eq!(written("\u{FEFF}[core]\nworkers = 3\n"), 3, "开头有 BOM");
    // 别的项、不认识的键不碍事：只认这一项。
    assert_eq!(
        written("[log]\nlevel = \"debug\"\n[core]\nidle_exit = 5\nworkers = 3\nwho = 1\n"),
        3
    );
}

#[test]
fn a_broken_value_falls_back_to_the_cores() {
    for text in [
        "",
        "[core]\nworkers = 1000\n",
        "[core]\nworkers = 65\n",
        "[core]\nworkers = -1\n",
        "[core]\nworkers = \"8\"\n",
        "[core]\nworkers = 3.5\n",
        "[core]\nworkers = 3\n[core\n",
    ] {
        assert_eq!(written(text), 0, "{text:?}");
        assert_eq!(pick(written(text), cores(12)), 12, "{text:?}");
    }
}

#[test]
fn a_missing_or_unreadable_file_falls_back_to_the_cores() {
    let dir = std::env::temp_dir().join(format!("miyu-workers-{}", std::process::id()));
    let root = DataRoot::at(dir.clone());
    let cores = std::thread::available_parallelism().map_or(FEWEST, NonZeroUsize::get);
    assert_eq!(count(&root), cores.clamp(FEWEST, MOST), "没有配置文件");
    let config = root.config_file();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[core]\nworkers = 3\n").unwrap();
    assert_eq!(count(&root), 3);
    std::fs::write(&config, [0xFF, 0xFE, 0x00]).unwrap();
    assert_eq!(count(&root), cores.clamp(FEWEST, MOST), "不是 UTF-8");
    std::fs::remove_dir_all(&dir).unwrap();
}
