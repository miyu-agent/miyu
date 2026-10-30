//! `grep`（`10-自带软件.md` 第三节、第十节，施工 4-4 下）：照正则搜文件内容，遵守 `.gitignore`。参数照 Claude Code
//! 的常用几个；三种输出：只列文件（默认）、匹配的行、每个文件几处；照 `head_limit`、`offset` 分页。
//!
//! 怎么搜在 [`search`]，怎么写给她看在 [`render`]；走目录、比模式和 `glob` 共用（[`crate::walk`]、
//! [`crate::pattern`]）。

mod render;
mod search;

use std::path::{Path, PathBuf};

use serde::Deserialize;

use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Progress, Running, Spec, Stop, Target, Tool};

use crate::blocking::blocking;
use crate::common::{Common, OUTPUT_BYTES, Shown, given, integer, said};
use crate::load::{self, LoadError, say};
use crate::pattern::Pattern;
use crate::walk;

/// 不给 `head_limit` 时最多几条（Claude Code 的默认）。
pub(crate) const HEAD_LIMIT: usize = 250;
/// 一行最长多少个字，多的截掉、补一个 `…`（pi 的做法）。
pub(crate) const LINE_CHARS: usize = 500;

/// `grep`。
pub(crate) struct Grep {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/grep/*.txt`，和几件工具共用的。
#[derive(Clone)]
pub(crate) struct Texts {
    common: Common,
    no_matches: Template,
    more_files: Template,
    more_matches: Template,
    past_end: Template,
    bad_pattern: Template,
}

/// 三种输出。
#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    /// 匹配的行。
    Content,
    /// 只列文件。
    #[default]
    FilesWithMatches,
    /// 每个文件几处。
    Count,
}

/// 她给的参数。名字照 Claude Code；它另有的 `-A`、`-B` 不声明，写了照样认；`-C` 当 `context`，opencode 的
/// `include` 当 `glob`（施工 4-4 下）。这几个写成字符串的整数也认（施工 4-9 再补二）：没声明的别名内核修正不到。
#[derive(Deserialize)]
struct Args {
    pattern: String,
    path: Option<String>,
    #[serde(alias = "include")]
    glob: Option<String>,
    output_mode: Option<Mode>,
    #[serde(rename = "-i")]
    ignore_case: Option<bool>,
    #[serde(alias = "-C", default, deserialize_with = "integer")]
    context: Option<i64>,
    #[serde(rename = "-A", default, deserialize_with = "integer")]
    after: Option<i64>,
    #[serde(rename = "-B", default, deserialize_with = "integer")]
    before: Option<i64>,
    head_limit: Option<i64>,
    offset: Option<i64>,
}

/// 照参数定下的一页：跳过几条，最多几条；匹配的行前后各带几行。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Page {
    pub(crate) offset: usize,
    pub(crate) limit: usize,
    pub(crate) before: usize,
    pub(crate) after: usize,
}

impl Args {
    /// 搜的地方：她给的写法，没给就是工作目录。
    fn path(&self) -> String {
        given(self.path.clone()).unwrap_or_else(|| ".".to_string())
    }

    /// 这一页。`context` 在的时候压过 `-A`、`-B`（Claude Code 的规矩）；负数当没给；`head_limit` 是 0 就不限。
    fn page(&self) -> Page {
        let count = |value: Option<i64>| value.and_then(|value| usize::try_from(value).ok());
        let limit = match count(self.head_limit) {
            None => HEAD_LIMIT,
            Some(0) => usize::MAX,
            Some(limit) => limit,
        };
        let (before, after) = match count(self.context) {
            Some(both) => (both, both),
            None => (
                count(self.before).unwrap_or(0),
                count(self.after).unwrap_or(0),
            ),
        };
        Page {
            offset: count(self.offset).unwrap_or(0),
            limit,
            before,
            after,
        }
    }
}

impl Grep {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Grep, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "grep", name, fields);
        Ok(Grep {
            spec: load::spec(resources, "grep", Access::Read)?,
            texts: Texts {
                common,
                no_matches: text("no-matches", &[])?,
                more_files: text("more-files", &["from", "to", "total", "next"])?,
                more_matches: text("more-matches", &["from", "to", "next"])?,
                past_end: text("past-end", &["total", "offset"])?,
                bad_pattern: text("bad-pattern", &["error"])?,
            },
        })
    }
}

impl Tool for Grep {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .map(|args| {
                vec![Target {
                    path: args.path(),
                    write: false,
                    itself: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => {
                    blocking(call.stop.clone(), move |stop| {
                        grep(&texts, &call, &args, stop)
                    })
                    .await
                }
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 搜：认好正则和通配，搜的地方换成真实的位置；是目录就往下走，是文件就只搜它。
fn grep(texts: &Texts, call: &Call, args: &Args, stop: &Stop) -> Done {
    let matcher = match search::matcher(&args.pattern, args.ignore_case.unwrap_or(false)) {
        Ok(matcher) => matcher,
        Err(error) => {
            return Done::error(say(&texts.bad_pattern, &[("error", &error)]))
                .said(said("grep/bad-pattern").with("error", error));
        }
    };
    let filter = match given(args.glob.clone()).map(|glob| (Pattern::new(&glob), glob)) {
        None => None,
        Some((Ok(pattern), _)) => Some(pattern),
        Some((Err(error), glob)) => return texts.common.bad_glob(&glob, &error),
    };
    let shown = Shown::here(call);
    let path = args.path();
    let real = match call.real(&path) {
        Ok(real) => real,
        Err(error) => return texts.common.failed(&path, &error),
    };
    let dir = match std::fs::metadata(&real) {
        Ok(meta) => meta.is_dir(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return texts.common.missing(&path, &real, &shown);
        }
        Err(error) => return texts.common.failed(&path, &error),
    };
    let place = Place {
        real,
        dir,
        filter: filter.as_ref(),
    };
    let page = args.page();
    match args.output_mode.unwrap_or_default() {
        Mode::FilesWithMatches => {
            let mut searcher = search::searcher(0, 0);
            let found: Vec<PathBuf> = place
                .matching(call, stop, |file| {
                    search::matches(&mut searcher, &matcher, file).then_some(())
                })
                .into_iter()
                .map(|(file, ())| file)
                .collect();
            render::files(texts, &shown, &found, page)
        }
        Mode::Count => {
            let mut searcher = search::searcher(0, 0);
            let counted = place.matching(call, stop, |file| {
                search::count(&mut searcher, &matcher, file)
            });
            render::counts(texts, &shown, &counted, page)
        }
        Mode::Content => {
            let files = place.files(call, stop);
            // 一次最多写 64 KiB，一行至少一个字节：不限的也用不着收多过这么多行。多收一行，才知道后面还有没有。
            let need = page
                .offset
                .saturating_add(page.limit.min(OUTPUT_BYTES))
                .saturating_add(1);
            let lines = search::lines(&matcher, &files, page, need, stop);
            render::lines(texts, &shown, &lines, page)
        }
    }
}

/// 搜的地方：真实的位置，是不是目录，照哪个通配挑文件。
struct Place<'a> {
    real: PathBuf,
    dir: bool,
    filter: Option<&'a Pattern>,
}

impl Place<'_> {
    /// 目录下面通配留下的文件，新的在前（[`walk::files`]）；点名的一个文件只有它：照搜，不管通配，ripgrep 也是这样。
    fn files(&self, call: &Call, stop: &Stop) -> Vec<PathBuf> {
        if !self.dir {
            return vec![self.real.clone()];
        }
        walk::files(&self.real, walk::Fence::of(call), stop, |relative| {
            self.keeps(relative)
        })
        .into_iter()
        .map(|found| found.path)
        .collect()
    }

    /// 同 [`Place::files`]，只是边走边照 `probe` 搜，只留搜到了的（`probe` 交回 `Some`），排好再交回（施工 V-21）：从家
    /// 目录搜下来不先把几百万个文件都记下，留下的只有匹配的那几个。交回的和先走完、排好、再一个个搜的一样。
    fn matching<T>(
        &self,
        call: &Call,
        stop: &Stop,
        mut probe: impl FnMut(&Path) -> Option<T>,
    ) -> Vec<(PathBuf, T)> {
        if !self.dir {
            return probe(&self.real)
                .map(|found| (self.real.clone(), found))
                .into_iter()
                .collect();
        }
        let mut found = Vec::new();
        walk::each(
            &self.real,
            walk::Fence::of(call),
            stop,
            |relative| self.keeps(relative),
            |file| {
                if let Some(result) = probe(&file.path) {
                    found.push((file, result));
                }
            },
        );
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
            .into_iter()
            .map(|(file, result)| (file.path, result))
            .collect()
    }

    /// 通配留不留这个相对路径：没给通配的都留。
    fn keeps(&self, relative: &str) -> bool {
        self.filter.is_none_or(|filter| filter.matches(relative))
    }
}
