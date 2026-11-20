## 施工单 HZ-2：删 mktemp 刚建的临时目录不问

状态：已完成（2026-11-12）。项目主人 2026-11-12 定方向，core1 加安全边界。

### 目的

主人的会话里 `tmp=$(mktemp -d /tmp/probe.XXXXXX); TMPDIR=$tmp cargo test …; rm -rf $tmp` 被问了三次，判的是 `delete_unresolved`：`$tmp` 来自命令替换，静态看不出删哪儿。同一条命令里写死的赋值（`T=/tmp/x; rm -rf $T`，施工 5-17 再补）早就认得。`mktemp` 每次新建一个唯一的空路径，同一条命令里删的就是它自己刚建的；这是写测试、脚本最常见的写法，每次都问只会让人习惯性点允许。这一步让它不问。放宽的是权限，拿不准的一律照旧判。

### 蓝图改哪几节

- `tools/shell.md`「高危」第 4 条：赋值那一款下面加一款，写认的写法和边界。
- `session/guard.md`「在哪」的 `hazard` 那一行多 `hazard/fresh.rs`，「守着它的」多 `hazard/tests/fresh.rs`。

### 定了的

**认的**。单独成句的 `x=$(mktemp …)`、`` x=`mktemp …` ``（`export`、`local`、`readonly`、`declare`、`typeset` 不带选项的也算）以后，`rm`（`rmdir`、`unlink`、`shred` 同一条路）的目标去掉引号以后正好是 `$x`、`${x}` 的不判。`mktemp` 认：不带参数、`-d`、`-u`、`-q`、`-t`（可以连写成 `-dt`）、`-p 目录`、`--directory`、`--dry-run`、`--quiet`、`--tmpdir`、`--tmpdir=目录`，加至多一个模板。

**边界和理由**。

| 边界 | 理由 |
|---|---|
| 只认不带后缀的引用；`$x/*`、`"$x"/`、`$x/..`、`${x}foo`、`${x:-…}` 照原规矩判 | `mktemp` 失败时 `x` 是空的：`rm -rf $x/*` 就是 `rm -rf /*`，`${x:-/}` 就是 `/`；空的 `$x` 什么也删不到 |
| 赋值正好是一个命令替换；`x=$(mktemp -d)/a`、`x=pre$(mktemp)` 不算 | 拼上的字可能把路径引到别处 |
| 替换里正好一条 `mktemp`：没有第二条命令（`; echo /`、`&& cd /`）、管道、重定向（`2>&1` 会把出错的话当路径）、`&`、套着的替换；`mktemp` 是光名字（`./mktemp`、`env mktemp` 不算） | 印出来的才是 `x` 的值，多一条命令就多一个来源 |
| 参数写死：有变量、命令替换（`-p $D`）、通配、空白的不算；不认的选项（`--suffix`）不算 | 运行时才知道的目录、会被 shell 拆开或展开的模板，不再是「一个新建的空路径」 |
| 赋值一定在这个 shell 里跑到：不在括号、花括号、`if`、`case`、循环里，前面不是 `&&`、`||`，后面不是 `&`，不在管道里，不是只给一个命令的 | 没跑到时 `x` 还是外面带进来的值（比如环境里同名的变量） |
| 赋值以后、删以前改过 `x` 的不算；删以后才改的不影响前面那次 | 改过就不是刚建的那个了 |
| 不照先后跑的改法整层不认：块里（循环、函数、`if`）任何一处改 `x`；原文任何一处 `${x:=…}` 这类带修饰的（heredoc 正文里的也算）、`((…))`、`$((…))` 里出现 `x`；这一层有 `eval`、`source`、`.`、`trap`、`alias`、`case`、`IFS` 的整层什么名字都不认 | 循环第二圈、函数调用、`trap` 会在字面的「之前」改它；`case` 的模式 `a)` 让括号数不准；`IFS` 改了没加引号的 `$x` 拆法 |
| 只认同一条命令里的：替换里、`sh -c` 里另起一份，上一次调用里赋的不算 | 子 shell、别的进程，赋值传不过来 |
| `find $x -delete` 照旧 | `x` 空了 `find` 就删当前目录 |
| 同一个 `rm` 里别的目标照原规矩各判各的，最严者胜 | 只放这一个目标 |

**怎么判「中间改过」**。照先后走每条管道：一个简单命令有词以 `x=`、`x+=`、`x[` 开头（赋值、`export x=…`、`let x=1`、只给一个命令的 `x=/ true` 也算），或者它是 `read`、`mapfile`、`readarray`、`unset`、`getopts`、`printf`、`let`、`declare`、`typeset`、`local`、`export`、`readonly`、`wait`、`for`、`select`、后面的词里出现 `x` 这个名字（不是 `$x`，`declare -n r=x` 也算），就算改过，忘掉。宁可多忘：`export x` 没改值也忘。

**顺手堵上的**。拆命令时替换只留一个 `$`，`$(echo /)x` 和 `$x` 拆出来字一样。带命令替换的词不当 `$x`，也不照记下的变量换：原来 `T=/tmp/zt; rm -rf $(echo /)T` 照删 `/tmp/zt` 放过去了（真删的是 `/T`），这一步一起改成照旧问。为此 `lex.rs` 的词记下每一段替换（`Word::substitutions`），每条管道记下是不是一定跑到（`Parsed::plain`）。

### 不做什么

- 不管没加引号的 `$x` 被 shell 按空白再拆：`TMPDIR` 里有空白时 `rm -rf $x` 删的是拆开的几段。Linux、macOS 上 `TMPDIR` 照惯例没有空白；写死的赋值（5-17 再补）也一样不管。
- 不放循环里新建又删的（`while …; do x=$(mktemp -d); …; rm -rf $x; done`）：赋值在块里，照旧问。
- 不认 `mktemp` 以外的命令替换。
- 不改给她看的字、问人时的说明。施工图不画。

### 验收

- `crates/miyu-policy/src/hazard/tests/fresh.rs`：主人那条原命令不问（先证明不改时是红的：`delete_unresolved`）；`"$x"`、`${x}`、`"${x}"`、反引号、`-u`、`-p`、`-t`、`--tmpdir` 不问；规则 2、3、4、6、7 的每一种反例照旧判（`rm -rf $x/*`、`"$x"/`、`x=$(mktemp -d)/a`、`x=$(mktemp -d); x=/`、`$(mktemp -d; echo /)`、`mktemp -p $D` 等）；`$(echo /)x` 不当 `$x`。
- `hazard/tests.rs` 原来写着 `T=$(mktemp -d); rm -rf $T` 照问的两条改成不问。
- `cargo test -p miyu-policy`、`miyu-basesystem`、`miyu-session`；两个目标的 clippy；`cargo fmt --check`；`cargo doc`；`cargo xtask prompts`（没有差异）。

人工走一遍：完全放开下让她跑 `tmp=$(mktemp -d); ls $tmp; rm -rf $tmp`，不弹确认；`rm -rf $tmp/*`、`rm -rf $(pwd)` 照旧弹。

### 风险

- 放宽的是权限：判错了就是不问人直接删。每一条边界都有反例测试守着，拿不准的照旧判。
- 认不准的写法照旧问，比如循环里新建又删；嫌烦的另报。
