//! 量尺（施工 VQ-1，`docs/construction/VQ-1-向量检索量一次.md`）：照意思检索（`RecallIndex::nearest`，`docs/blueprint/recall.md`
//! 第三条第 2 款）在 661、5035、5 万、50 万条时查一次要多久，分开量「从库里读出来」「算」「排」「取字」四段，热的（页缓存里
//! 有）和冷的（清掉这几个文件的系统页缓存）各 20 次取 p50、p95，再记头一次查时进程常驻内存涨了多少；另量一个对照：向量整块留在
//! 内存里扫一遍要多久（不是生产的做法，给要不要改一个数）。只断言结果，不断言耗时。
//!
//! 向量是造的：随机、384 维、归一化（照出厂的 granite-embedding-97m-multilingual-r2，`recall.md` 第四条第 2 款），不读真数据。
//! 库照生产的路子建：一个会话 100 轮，`apply` 一批放进条目（连同 FTS5 的词），再一条一条 `put_vector`，和后台补向量一样。
//!
//! 清页缓存用 GNU `dd` 的 `oflag=nocache`（`posix_fadvise` 的 `DONTNEED`，不要 root），量内存读 `/proc/self/status` 的
//! `VmHWM`（量之前写 `/proc/self/clear_refs` 清零），真从盘上读了多少看 `/proc/self/io` 的 `read_bytes`：都只有 Linux 有，
//! 量的又是整个进程，所以自己一个测试程序（AGENTS.md「测试」），只在 Linux 上编。
//!
//! 两步，量的一档一个进程（分配器留着前面放掉的内存，后面的查拿来接着用，常驻内存就涨得少了：建库、量上一档时分出去的
//! 都不能在同一个进程里）：
//!
//! ```text
//! MIYU_VQ1_DIR=<不是内存盘的目录> cargo test -p miyu-store --release --test recall_vectors_measure -- --ignored --exact build_vector_indexes
//! MIYU_VQ1_DIR=<同一个目录> MIYU_VQ1_SIZES=<一档> cargo test -p miyu-store --release --test recall_vectors_measure -- --ignored --exact measure_vector_search --nocapture
//! ```
//!
//! `MIYU_VQ1_SIZES` 是量哪几档（逗号分开，默认 `661,5035,50000,500000`）。50 万条的库约 3.1 GB，量完自己删掉那个目录。
#![cfg(target_os = "linux")]

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use miyu_kernel::id::Seq;
use miyu_recall::vector::{dot, from_bytes};
use miyu_store::recall::{Edit, RecallIndex};
use rusqlite::{Connection, OptionalExtension};

use support::{at, root_at, session};

/// 出厂语义模型的编号和维数。
const MODEL: &str = "local:granite-embedding-97m-multilingual-r2";
const DIMS: usize = 384;
/// 一次取几条：记忆主动搜 10 条的四倍（`keeper/search.rs`），三处用法（回合 20、知识库 32、记忆 40）里最多的。
const WIDE: usize = 40;
/// 每一档量几次。
const RUNS: usize = 20;
/// 一个会话几轮。
const TURNS: u64 = 100;

/// 放库的目录：`MIYU_VQ1_DIR`，不写的放系统临时目录（常是内存盘，冷的量不出来）。
fn dir() -> PathBuf {
    std::env::var_os("MIYU_VQ1_DIR")
        .map_or_else(|| std::env::temp_dir().join("miyu-vq1"), PathBuf::from)
}

/// 量哪几档。
fn sizes() -> Vec<usize> {
    let text = std::env::var("MIYU_VQ1_SIZES").unwrap_or_else(|_| "661,5035,50000,500000".into());
    text.split(',')
        .map(|n| n.trim().parse().expect("规模是整数"))
        .collect()
}

/// `n` 条的那一份库在哪：目录下一个数据根（写上标记，`support::Scratch` 的做法），量完不删，下次照用。
fn place(n: usize) -> PathBuf {
    let dir = dir();
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(
        dir.join(miyu_store::root::MARKER),
        miyu_store::root::MARKER_TEXT,
    )
    .expect("写得了");
    root_at(&dir.join("data"))
        .index()
        .join("recall")
        .join(format!("vq1-{n}.db"))
}

/// 可重现的随机数（splitmix64）。
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 一个归一化过的随机向量。
    fn vector(&mut self) -> Vec<f32> {
        let raw: Vec<f32> = (0..DIMS)
            .map(|_| (self.next() >> 40) as f32 / (1u64 << 23) as f32 - 1.0)
            .collect();
        let norm = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
        raw.into_iter().map(|x| x / norm).collect()
    }
}

/// 第 `n` 轮的字：约 300 个汉字（一轮人说的加她回的），从一小撮词里挑，FTS5 的词表不至于大得离谱。
fn turn_text(n: usize) -> String {
    const WORDS: [&str; 20] = [
        "今天",
        "明天",
        "周末",
        "我们",
        "一起",
        "去看",
        "樱花",
        "爬山",
        "写代码",
        "编辑器",
        "考试",
        "显卡",
        "猫咪",
        "吃饭",
        "火锅",
        "电影",
        "游戏",
        "加班",
        "睡觉",
        "下雨",
    ];
    (0..150)
        .map(|k| WORDS[(n * 7 + k * 13 + n / 3) % WORDS.len()])
        .collect()
}

/// 建 `n` 条的库；已经建好的（`n` 条、条条有向量）不动。
fn build(n: usize) {
    let path = place(n);
    let (index, _) = RecallIndex::open(&path);
    if index.coverage(MODEL).expect("读得了") == (n as u64, n as u64) {
        println!("{n}: 已经有了，不重建");
        return;
    }
    drop(index);
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.clone().into_os_string();
        name.push(suffix);
        if Path::new(&name).exists() {
            std::fs::remove_file(&name).expect("删得了");
        }
    }
    let (index, _) = RecallIndex::open(&path);
    let started = Instant::now();
    let mut keys = Vec::with_capacity(n);
    for (s, first) in (0..n).step_by(TURNS as usize).enumerate() {
        let source = session(u32::try_from(s).expect("会话不多")).to_string();
        let edits: Vec<Edit> = (first..n.min(first + TURNS as usize))
            .map(|k| {
                let key = format!("{source}/{}", k - first + 1);
                keys.push(key.clone());
                Edit::Put {
                    key,
                    text: turn_text(k),
                    at: at(k as i64),
                }
            })
            .collect();
        index
            .apply(&source, &edits, Seq::new(TURNS).expect("从 1 起"))
            .expect("写得进");
    }
    let items = started.elapsed();
    let mut random = Random(n as u64);
    for key in &keys {
        index
            .put_vector(key, MODEL, &random.vector())
            .expect("写得进");
    }
    assert_eq!(index.coverage(MODEL).expect("读得了"), (n as u64, n as u64));
    println!(
        "{n}: 条目 {items:?}，向量 {:?}，库 {} MiB",
        started.elapsed() - items,
        size(&path) / (1 << 20)
    );
}

/// 库连同 `-wal` 多少字节。
fn size(path: &Path) -> u64 {
    ["", "-wal"]
        .iter()
        .map(|suffix| {
            let mut name = path.to_path_buf().into_os_string();
            name.push(suffix);
            std::fs::metadata(name).map_or(0, |meta| meta.len())
        })
        .sum()
}

/// 清掉库、`-wal` 在系统页缓存里的页：先落盘，再 `DONTNEED`。
fn drop_cache(path: &Path) {
    for suffix in ["", "-wal"] {
        let mut name = path.to_path_buf().into_os_string();
        name.push(suffix);
        if !Path::new(&name).exists() {
            continue;
        }
        let mut of = std::ffi::OsString::from("of=");
        of.push(&name);
        let status = Command::new("dd")
            .arg(of)
            .args([
                "oflag=nocache",
                "conv=notrunc,fdatasync",
                "count=0",
                "status=none",
            ])
            .status()
            .expect("有 GNU dd");
        assert!(status.success(), "dd 清不掉 {name:?} 的页缓存");
    }
}

/// `/proc/self/status` 里一格，字节。
fn status(field: &str) -> u64 {
    let text = std::fs::read_to_string("/proc/self/status").expect("读得了");
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .expect("有这一格");
    line.trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<u64>()
        .expect("是数")
        * 1024
}

/// 这个进程真从盘上读了多少字节（`/proc/self/io` 的 `read_bytes`）。
fn disk_read() -> u64 {
    let text = std::fs::read_to_string("/proc/self/io").expect("读得了");
    text.lines()
        .find_map(|line| line.strip_prefix("read_bytes:"))
        .and_then(|rest| rest.trim().parse().ok())
        .expect("有 read_bytes")
}

/// 一次分段的查：读、算（解字节加点积）、排、取字各多久，交回最像的那几个键（和 `nearest` 对得上才算同一件事）。语句、算法和
/// `vectors.rs` 的 `nearest` 一样；不同的只有几段分开做（先把整列读进内存，再算），量的是各占多少。
fn split(db: &Connection, query: &[f32]) -> ([Duration; 4], Vec<String>) {
    let started = Instant::now();
    let mut select = db
        .prepare("SELECT key, data FROM vectors WHERE model = ?1")
        .expect("读得了");
    let rows: Vec<(String, Vec<u8>)> = select
        .query_map([MODEL], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("读得了")
        .collect::<Result<_, _>>()
        .expect("读得了");
    let read = started.elapsed();
    let started = Instant::now();
    let mut scored: Vec<(f32, String)> = rows
        .into_iter()
        .filter_map(|(key, data)| Some((dot(query, &from_bytes(&data)?), key)))
        .collect();
    let score = started.elapsed();
    let started = Instant::now();
    scored.sort_by(|(a, key_a), (b, key_b)| b.total_cmp(a).then(key_a.cmp(key_b)));
    scored.truncate(WIDE);
    let sort = started.elapsed();
    let started = Instant::now();
    for (_, key) in &scored {
        let item: Option<(String, i64)> = db
            .query_row("SELECT text, at FROM items WHERE key = ?1", [key], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()
            .expect("读得了");
        assert!(item.is_some(), "条目在");
    }
    let texts = started.elapsed();
    (
        [read, score, sort, texts],
        scored.into_iter().map(|(_, key)| key).collect(),
    )
}

/// 对照「向量留在内存里」（建议里的第二种，不是生产的做法）：整列读一次，排成一整块 f32，查的时候只扫这一块。
struct Resident {
    keys: Vec<String>,
    data: Vec<f32>,
}

impl Resident {
    fn load(db: &Connection) -> Resident {
        let mut select = db
            .prepare("SELECT key, data FROM vectors WHERE model = ?1")
            .expect("读得了");
        let mut rows = select.query([MODEL]).expect("读得了");
        let (mut keys, mut data) = (Vec::new(), Vec::new());
        while let Some(row) = rows.next().expect("读得了") {
            keys.push(row.get(0).expect("是字"));
            let bytes: Vec<u8> = row.get(1).expect("是字节");
            data.extend(from_bytes(&bytes).expect("长度对"));
        }
        Resident { keys, data }
    }

    /// 扫一遍：八路累加的点积（编译器照它用 SIMD），只挑前 [`WIDE`] 条再排，不排整列。交回那几个键。
    fn scan(&self, query: &[f32]) -> Vec<String> {
        let dot8 = |row: &[f32]| {
            let mut sum = [0.0f32; 8];
            for (a, b) in row.chunks_exact(8).zip(query.chunks_exact(8)) {
                for lane in 0..8 {
                    sum[lane] += a[lane] * b[lane];
                }
            }
            sum.iter().sum::<f32>()
        };
        let mut scored: Vec<(f32, u32)> = self
            .data
            .chunks_exact(DIMS)
            .zip(0u32..)
            .map(|(row, at)| (dot8(row), at))
            .collect();
        let wide = WIDE.min(scored.len());
        if wide < scored.len() {
            scored.select_nth_unstable_by(wide, |(a, _), (b, _)| b.total_cmp(a));
        }
        scored.truncate(wide);
        scored.sort_by(|(a, _), (b, _)| b.total_cmp(a));
        scored
            .into_iter()
            .map(|(_, at)| self.keys[at as usize].clone())
            .collect()
    }
}

/// 和生产的读连接一样开（`recall/conn.rs` 的 `both`）：默认的页缓存、只读。
fn reader(path: &Path) -> Connection {
    let db = Connection::open(path).expect("开得了");
    db.pragma_update(None, "query_only", true).expect("设得了");
    db
}

/// 几次里的 p50、p95（取第 ⌈q·n⌉ 个），毫秒。
fn quantiles(times: &mut [Duration]) -> (f64, f64) {
    times.sort();
    let pick = |q: f64| {
        let at = ((q * times.len() as f64).ceil() as usize).clamp(1, times.len()) - 1;
        times[at].as_secs_f64() * 1000.0
    };
    (pick(0.50), pick(0.95))
}

/// 一列：p50 / p95。
fn cell(times: &mut [Duration]) -> String {
    let (p50, p95) = quantiles(times);
    format!("{p50:.2} / {p95:.2}")
}

/// 生产的 `nearest` 查一次：多久、常驻内存比查之前涨了多少（峰值）、真从盘上读了多少；查出的键。
fn whole(index: &RecallIndex, query: &[f32]) -> (Duration, u64, u64, Vec<String>) {
    std::fs::write("/proc/self/clear_refs", "5").expect("清得了峰值");
    let (resident, disk) = (status("VmRSS:"), disk_read());
    let started = Instant::now();
    let near = index.nearest(MODEL, query, WIDE).expect("读得了");
    let took = started.elapsed();
    let grown = status("VmHWM:").saturating_sub(resident);
    assert_eq!(near.len(), WIDE);
    (
        took,
        grown,
        disk_read() - disk,
        near.into_iter().map(|hit| hit.key).collect(),
    )
}

/// 量一档：热的、冷的各 [`RUNS`] 次，印一行表。
fn measure(n: usize, random: &mut Random) {
    let path = place(n);
    let (index, _) = RecallIndex::open(&path);
    assert_eq!(
        index.coverage(MODEL).expect("读得了"),
        (n as u64, n as u64),
        "先跑 build_vector_indexes"
    );
    let (mut hot, mut cold, mut kept) = (Vec::new(), Vec::new(), Vec::new());
    let (mut hot_parts, mut cold_parts) = ([const { Vec::new() }; 4], [const { Vec::new() }; 4]);
    let (mut grown, mut cold_read) = (None, Vec::new());
    // 冷的：每次先清页缓存。生产的读连接一直开着，它自己的页缓存（默认 2 MiB）留着，照实；分段的每次新开一个连接，全冷。
    for _ in 0..RUNS {
        drop_cache(&path);
        let (took, grew, read, _) = whole(&index, &random.vector());
        cold.push(took);
        cold_read.push(read);
        grown.get_or_insert(grew);
        drop_cache(&path);
        let (parts, _) = split(&reader(&path), &random.vector());
        for (all, part) in cold_parts.iter_mut().zip(parts) {
            all.push(part);
        }
    }
    // 热的：先各空跑一次把页缓存读满。
    let db = reader(&path);
    let resident = Resident::load(&db);
    whole(&index, &random.vector());
    split(&db, &random.vector());
    for _ in 0..RUNS {
        let query = random.vector();
        let (took, _, _, keys) = whole(&index, &query);
        hot.push(took);
        let (parts, same) = split(&db, &query);
        assert_eq!(keys, same, "分段的和生产的查出一样的几条");
        for (all, part) in hot_parts.iter_mut().zip(parts) {
            all.push(part);
        }
        let started = Instant::now();
        let mut found = resident.scan(&query);
        kept.push(started.elapsed());
        // 八路累加的和差在末几位，同分附近的先后可能换，只比是哪几条。
        let mut keys = keys;
        found.sort();
        keys.sort();
        assert_eq!(found, keys, "留在内存里扫的查出同样的几条");
    }
    cold_read.sort_unstable();
    let [read, score, sort, texts] = &mut hot_parts;
    let [cold_read_part, _, _, cold_texts] = &mut cold_parts;
    println!(
        "| {n} | {} MiB | {} | {} | {} | {} | {} | {} | {} | {} | {} MiB | {:.1} MiB | {} | {} MiB |",
        size(&path) / (1 << 20),
        cell(&mut hot),
        cell(read),
        cell(score),
        cell(sort),
        cell(texts),
        cell(&mut cold),
        cell(cold_read_part),
        cell(cold_texts),
        cold_read[RUNS / 2] / (1 << 20),
        grown.unwrap_or(0) as f64 / f64::from(1 << 20),
        cell(&mut kept),
        resident.data.len() * 4 / (1 << 20),
    );
}

/// 第一步：建库（单独一个进程跑，建的时候分出去的内存不算进量的那一次）。
#[test]
#[ignore = "量尺"]
fn build_vector_indexes() {
    for n in sizes() {
        build(n);
    }
}

/// 第二步：量。表的每格是毫秒的 p50 / p95（冷的算、排和热的一样，不另列）；「留内存」两列是对照：向量整块留在内存里扫一遍
/// 要多久、占多少（不是生产的做法）；「冷读盘」是冷的那几次生产的查真从盘上读的中位数，「内存」是这一档头一次查（冷的）
/// 常驻内存的峰值比查之前涨了多少。
#[test]
#[ignore = "量尺"]
fn measure_vector_search() {
    println!(
        "| 条数 | 库 | 热·整次 | 热·读 | 热·算 | 热·排 | 热·取字 | 冷·整次 | 冷·读 | 冷·取字 | 冷读盘 | 内存 | 留内存·扫 | 留内存·占 |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut random = Random(7);
    for n in sizes() {
        measure(n, &mut random);
    }
}
