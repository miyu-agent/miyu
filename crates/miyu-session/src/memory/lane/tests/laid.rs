//! 回合索引接上时边读边铺（施工 V-37 下）：和原来整份读出来再铺的（[`old`]）各在一个数据根里接同一份日志，回合库里写进去的
//! 每一行（条目的键、字、时刻，墓碑，照到哪，用到的，场所，从哪来，连全文检索的表）逐行相等，接上以后的 `upto`、读不了时
//! 报的错一样。日志照种子造（[`Script`]）：撤销、恢复、中途换人格、群里照几句开的、调搜记忆的；分几段；分叉出来的支线带
//! 底子（父会话过了分叉点又说了一截）；库里已经照到一半的；接上以后落一批恢复（不记得字，读回再算）的。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId, VenueId};
use miyu_store::recall::{Origin, RecallIndexes, Room};

use super::super::super::recast;
use super::super::{Laid, Open, Opening, Source, attach};
use super::old;
use super::script::{Dice, Script};
use crate::store::Base;

const SESSION: &str = "01a0f233-cfec-7023-8ed5-2a037a1d5f01";
const PARENT: &str = "01a0f233-cfec-7023-8ed5-2a037a1d5f02";

/// 一份要接的：日志怎么写、怎么接。
struct Case {
    /// 父会话的日志（从第 1 条起）和分叉点：分叉出来的支线才有。
    parent: Option<(Vec<String>, u64)>,
    /// 会话自己的日志，第一条的序号。
    own: Vec<String>,
    first: u64,
    /// 每一段有几行（最后一段装下剩下的）。
    cuts: Vec<usize>,
    /// 先照前几行接一次（库里照到一半），再写全了接。
    prefix: Option<usize>,
    /// 接上以后落的一批（写进日志，再交给接上了的那一条道）。
    batch: Vec<String>,
    /// 记在哪个人格那一间：换成它以前的并进底子。
    persona: &'static str,
    venue: &'static str,
    main: bool,
    /// 交整份事件接（换人格时读回的那一路），不是自己读日志。
    given: bool,
}

/// 一边（原来的、新的）接完以后：回合库导出来的每一行、`upto`、报的错。
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    rows: Vec<String>,
    upto: Option<Option<Seq>>,
    error: Option<String>,
}

fn events(lines: &[String]) -> Vec<Event> {
    lines
        .iter()
        .map(|line| Event::from_line(line).expect("造对了"))
        .collect()
}

/// 照 `cuts` 把 `lines`（第一条是第 `first` 条）分段写进 `dir`：一段的名字是它第一条的序号。原来有的先删掉。
fn write_log(dir: &Path, first: u64, lines: &[String], cuts: &[usize]) {
    drop(std::fs::remove_dir_all(dir));
    std::fs::create_dir_all(dir).expect("建得了");
    let mut at = 0;
    let mut sizes = cuts.to_vec();
    sizes.push(usize::MAX);
    for size in sizes {
        if at >= lines.len() && at > 0 {
            break;
        }
        let end = at.saturating_add(size).min(lines.len());
        let text: String = lines[at..end]
            .iter()
            .map(|line| format!("{line}\n"))
            .collect();
        let name = first + at as u64;
        std::fs::write(dir.join(format!("{name:012}.jsonl")), text).expect("写得进");
        at = end;
    }
}

/// 一边接：`connect` 是原来的或者新的，`advance` 同。
fn run(
    case: &Case,
    side: &str,
    connect: fn(Opening) -> Result<Open, miyu_store::log::OpenError>,
    advance: fn(&mut Open, Vec<Event>),
) -> Outcome {
    let (scratch, root) = super::root(side);
    let session = SessionId::parse(SESSION).expect("编号写对了");
    let dir = root.session_dir(&session);
    let base = case.parent.as_ref().map(|(lines, upto)| {
        let parent = root.session_dir(&SessionId::parse(PARENT).expect("编号写对了"));
        write_log(&parent, 1, lines, &case.cuts);
        Base::new(parent, Seq::new(*upto).expect("非零"))
    });
    let recall = Arc::new(RecallIndexes::new(&root));
    let room = Room::persona(case.persona);
    let opening = |own: &[String]| {
        // 换人格的界照载入时读到的（读不成的行不算：读不了的日志照样交给接上的去读、去报）。
        let readable: Vec<Event> = own
            .iter()
            .filter_map(|line| Event::from_line(line).ok())
            .collect();
        let floor = recast::floor(&readable, case.persona);
        let all = if case.given { events(own) } else { Vec::new() };
        Opening {
            recall: Arc::clone(&recall),
            room: room.clone(),
            session: session.clone(),
            origin: Origin { main: case.main },
            venue: VenueId::parse(case.venue).expect("场所写对了"),
            floor,
            laid: if case.given {
                Laid::Given(all)
            } else {
                Laid::Read(&[])
            },
            source: Source {
                dir: dir.clone(),
                base: base.clone(),
            },
        }
    };
    if let Some(prefix) = case.prefix {
        let own = &case.own[..prefix];
        write_log(&dir, case.first, own, &[]);
        drop(connect(opening(own)));
    }
    write_log(&dir, case.first, &case.own, &case.cuts);
    let outcome = match connect(opening(&case.own)) {
        Ok(mut opened) => {
            let upto = opened.upto;
            if !case.batch.is_empty() {
                let mut all = case.own.clone();
                all.extend(case.batch.iter().cloned());
                write_log(&dir, case.first, &all, &case.cuts);
                advance(&mut opened, events(&case.batch));
            }
            drop(opened);
            Outcome {
                rows: Vec::new(),
                upto: Some(upto),
                error: None,
            }
        }
        Err(error) => Outcome {
            rows: Vec::new(),
            upto: None,
            error: Some(error.to_string()),
        },
    };
    drop(recall);
    Outcome {
        rows: dump(&scratch.0),
        ..outcome
    }
}

/// 数据根里每一份回合库的每一张表的每一行，照库名、表名，一张表里的行排好。
fn dump(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    collect(dir, &mut found);
    found.sort();
    let mut rows = Vec::new();
    for db in found {
        let name = db
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        rows.push(format!("== {name}"));
        let conn = rusqlite::Connection::open(&db).expect("开得了");
        let mut tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .expect("合写法")
            .query_map([], |row| row.get(0))
            .expect("查得了")
            .map(|name| name.expect("读得出"))
            .collect();
        tables.sort();
        for table in tables {
            let mut statement = conn
                .prepare(&format!("SELECT * FROM \"{table}\""))
                .expect("合写法");
            let width = statement.column_count();
            let mut lines: Vec<String> = statement
                .query_map([], |row| {
                    (0..width)
                        .map(|k| row.get::<_, rusqlite::types::Value>(k))
                        .collect::<Result<Vec<_>, _>>()
                })
                .expect("查得了")
                .map(|row| format!("{:?}", row.expect("读得出")))
                .collect();
            lines.sort();
            rows.push(format!("-- {table} {}", lines.len()));
            rows.extend(lines);
        }
    }
    rows
}

/// `dir` 下面的回合库（`turns-<人格>.db`）。
fn collect(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            collect(&path, found);
        } else if name.starts_with("turns-") && name.ends_with(".db") {
            found.push(path);
        }
    }
}

/// 两边接同一份，比。临时目录照 `tag` 分开（几个测试一起跑）。
fn same(case: &Case, tag: &str, what: &str) -> Outcome {
    let old = run(case, &format!("{tag}-old"), old::connect, old::advance);
    let new = run(case, &format!("{tag}-new"), attach, Open::advance);
    assert_eq!(new, old, "{what}");
    new
}

/// 写了几条条目。
fn items(outcome: &Outcome) -> usize {
    outcome
        .rows
        .iter()
        .find_map(|row| row.strip_prefix("-- items "))
        .and_then(|count| count.parse().ok())
        .unwrap_or(0)
}

/// 照种子造一份。
fn random(dice: &mut Dice) -> Case {
    let mut script = Script::new(1);
    let forked = dice.below(4) == 0;
    let (parent, first) = if forked {
        let count = 5 + dice.below(60) as usize;
        let trunk = script.run(dice, count);
        let upto = script.next() - 1;
        let mut more = script.clone();
        let mut lines = trunk;
        let count = dice.below(20) as usize;
        lines.extend(more.run(dice, count));
        (Some((lines, upto)), upto + 1)
    } else {
        (None, 1)
    };
    let count = 1 + dice.below(200) as usize;
    let mut own = script.run(dice, count);
    let persona = ["a", "b", "nobody"][dice.below(3) as usize];
    if dice.below(6) == 0 {
        own.push(script.recast(if persona == "nobody" { "" } else { persona }));
    }
    let cuts: Vec<usize> = (0..dice.below(4))
        .map(|_| 1 + dice.below(80) as usize)
        .collect();
    let prefix =
        (dice.coin() && own.len() > 1).then(|| 1 + dice.below(own.len() as u64 - 1) as usize);
    let batch = match dice.below(3) {
        0 => script.restore(),
        1 => {
            let count = 1 + dice.below(10) as usize;
            script.run(dice, count)
        }
        _ => Vec::new(),
    };
    Case {
        parent,
        own,
        first,
        cuts,
        prefix,
        batch,
        persona: if persona == "nobody" {
            miyu_store::recall::NO_PERSONA
        } else {
            persona
        },
        venue: if dice.coin() { "local" } else { "qq:group:1" },
        main: dice.below(4) > 0,
        given: dice.below(5) == 0,
    }
}

#[test]
fn laying_piece_by_piece_writes_what_the_whole_log_did() {
    let mut dice = Dice(0x5eed_2026_1103);
    let (mut written, mut restored, mut forked, mut halfway) = (0, 0, 0, 0);
    for round in 0..160 {
        let case = random(&mut dice);
        let outcome = same(&case, "laid", &format!("第 {round} 份"));
        written += usize::from(items(&outcome) > 0);
        restored += usize::from(
            case.batch
                .iter()
                .any(|line| line.contains("turn.unreverted")),
        );
        forked += usize::from(case.parent.is_some());
        halfway += usize::from(case.prefix.is_some());
    }
    assert!(written > 60, "写了库的 {written}");
    assert!(restored > 20, "接上以后恢复的 {restored}");
    assert!(forked > 20, "分叉的 {forked}");
    assert!(halfway > 40, "库里照到一半的 {halfway}");
}

/// 换人格以后一条都没有的（最后一条就是换成这一间的人格）、新造的会话：不开库，两边一样什么库都没有。
#[test]
fn nothing_after_the_floor_opens_no_index_either_way() {
    let mut dice = Dice(0x0dd5_eed5);
    let mut script = Script::new(1);
    let mut own = script.run(&mut dice, 40);
    own.push(script.recast("a"));
    let case = Case {
        parent: None,
        own,
        first: 1,
        cuts: vec![7, 9],
        prefix: None,
        batch: Vec::new(),
        persona: "a",
        venue: "local",
        main: true,
        given: false,
    };
    let outcome = same(&case, "floor", "换人格以后一条都没有");
    assert!(outcome.rows.is_empty(), "{:?}", outcome.rows);
    assert_eq!(outcome.upto, Some(Seq::new(41)), "upto 照自己的最后一条");
}

/// 读不了的：自己的坏了、底子坏了、两样都坏了、父会话没写到分叉点，两边报的错一样（两样都坏了的报自己的），库一点都不碰。
#[test]
fn an_unreadable_log_is_told_alike_and_touches_no_index() {
    let mut dice = Dice(0xbad_10c);
    let mut script = Script::new(1);
    let trunk = script.run(&mut dice, 30);
    let own = script.run(&mut dice, 30);
    let broken = |lines: &[String], at: usize| {
        let mut lines = lines.to_vec();
        lines[at] = lines[at].replacen("\"kind\"", "\"kind", 1);
        lines
    };
    let case = |parent: Option<(Vec<String>, u64)>, own: Vec<String>| Case {
        parent,
        own,
        first: 31,
        cuts: vec![10],
        prefix: None,
        batch: Vec::new(),
        persona: "a",
        venue: "local",
        main: true,
        given: false,
    };
    let fine = Some((trunk.clone(), 30));
    for (what, parent, own) in [
        ("自己的坏了", fine.clone(), broken(&own, 12)),
        ("底子坏了", Some((broken(&trunk, 3), 30)), own.clone()),
        (
            "两样都坏了",
            Some((broken(&trunk, 3), 30)),
            broken(&own, 12),
        ),
        (
            "父会话没写到分叉点",
            Some((trunk[..20].to_vec(), 30)),
            own.clone(),
        ),
        (
            "父会话没写到分叉点、自己的也坏了",
            Some((trunk[..20].to_vec(), 30)),
            broken(&own, 2),
        ),
    ] {
        let outcome = same(&case(parent, own), "unreadable", what);
        assert!(outcome.error.is_some(), "{what}");
        assert!(outcome.rows.is_empty(), "{what}：{:?}", outcome.rows);
    }
    let outcome = same(&case(fine, own), "unreadable", "都读得了");
    assert!(outcome.error.is_none());
    assert!(!outcome.rows.is_empty(), "读得了的开了库");
}

/// 真数据的形状（`#[ignore]`）：`MIYU_V37_ROOT` 指一份数据根的副本，里面每个会话（分叉出来的带底子）照几间各接一遍，两边
/// 回合库逐行一样、`upto` 一样。只读那份数据根，回合库写在临时目录。施工时在合成的大会话、真跑出来的撤销、恢复、换人格、
/// 分叉的会话上跑，结果写进施工单。
#[test]
#[ignore = "要一份数据根的副本：MIYU_V37_ROOT"]
fn sessions_in_a_data_root_lay_alike() {
    use miyu_store::env::{Env, Platform};

    let Some(home) = std::env::var_os("MIYU_V37_ROOT") else {
        return;
    };
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(home),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let data = miyu_store::root::DataRoot::locate(&env).expect("找得到数据根");
    let mut compared = 0;
    for session in data.sessions().expect("列得出") {
        let dir = data.session_dir(&session);
        let Ok(first) = miyu_store::log::first_event(&dir) else {
            continue;
        };
        let miyu_kernel::event::Body::SessionCreated(created) = first.body else {
            continue;
        };
        let base = created
            .forked
            .as_ref()
            .map(|forked| Base::new(data.session_dir(&forked.from), forked.upto));
        let own = miyu_store::log::read_events(&dir).expect("读得了");
        for persona in [miyu_store::recall::NO_PERSONA, "engineer", "miyu"] {
            let floor = recast::floor(&own, persona);
            let side =
                |tag: &str, connect: fn(Opening) -> Result<Open, miyu_store::log::OpenError>| {
                    let (scratch, root) = super::root(tag);
                    let recall = Arc::new(RecallIndexes::new(&root));
                    let opened = connect(Opening {
                        recall: Arc::clone(&recall),
                        room: Room::persona(persona),
                        session: session.clone(),
                        origin: Origin {
                            main: created.parent.is_none(),
                        },
                        venue: created.venue.clone(),
                        floor,
                        laid: Laid::Read(&[]),
                        source: Source {
                            dir: dir.clone(),
                            base: base.clone(),
                        },
                    });
                    let upto = opened.as_ref().ok().map(|opened| opened.upto);
                    let error = opened.as_ref().err().map(ToString::to_string);
                    drop(opened);
                    drop(recall);
                    Outcome {
                        rows: dump(&scratch.0),
                        upto,
                        error,
                    }
                };
            let old = side("real-old", old::connect);
            let new = side("real-new", attach);
            assert_eq!(new, old, "{session} 记在 {persona} 那一间");
            compared += 1;
            eprintln!(
                "{session} {persona}：{} 条，库 {} 行，条目 {}",
                own.len(),
                new.rows.len(),
                items(&new)
            );
        }
    }
    eprintln!("比了 {compared} 处");
}
