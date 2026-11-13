## 权限策略

### 是什么

执行前那条链的默认实现，模块编号 `permissions`。工具报出这次调用要碰的路径，换成真实的位置、查边界表，每一条照实际生效的那一级判，合起来照最严的：有一条拒绝就拒绝，有一条要问人就问人。不报路径的调用，照它做什么判：执行命令，完全放开放行、不进沙盒；只读时沙盒能用就放行、在只读沙盒里跑（执行器写规格，`session/tools.md`），用不了的问人（施工 5-4 上；A16 起只剩只读、完全放开两级，施工 LV-1）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-session/src/guard.rs` | 判一次调用：实际生效的那一级、判法表、不报路径的、问人时提的规则和说明、拒绝 |
| `crates/miyu-fs/src/resolve.rs` | 换成真实的位置，`~` 接家目录（`fs.md`） |
| `crates/miyu-fs/src/boundary.rs` | 边界表：一个真实的位置落在哪一片（`fs.md`） |
| `crates/miyu-policy/src/guard.rs` | 拒绝时写给她的三句，造会话、载入时从策略快照里拿 |
| `resources/core/permissions/forbidden.txt`、`unresolvable.txt`，`resources/core/tool-results/read-only.txt` | 三句的原文 |
| `resources/core/permissions/not-in-venue.txt`（施工 O-31 前） | 本机的会话里拒绝在场所里做的事的那一句：不进策略快照，本机的会话造、载入时读（`crates/miyu-session/src/open.rs` 的 `place`） |
| `crates/miyu-session/src/guard/untargeted.rs` | 不报路径的调用（第四条）：照访问类别、在场所里做的事（施工 5-18 从 `guard.rs` 挪出来：那边到了 500 行） |
| `crates/miyu-session/src/guard/asking.rs` | 要问人的那几条：提的放行规则、给头看的说明、放行过的管不管得到（施工 X-10 上从 `guard.rs` 挪出来） |
| `crates/miyu-session/src/guard/confined.rs`、`resources/core/permissions/outside-workspace.txt`、`no-commands.txt`（施工 5-12） | 只碰得到自己工作区的会话（第四条末）：判在不在工作区、拒绝时的两句；两句同样不进策略快照，这种会话造、载入时读 |
| `crates/miyu-policy/src/hazard.rs`、`hazard/lex.rs`、`hazard/rules.rs`（施工 5-17）、`hazard/vars.rs`（施工 5-17 再补：同一条命令里赋过值的变量）、`hazard/fresh.rs`（施工 HZ-2：同一条命令里 `mktemp` 刚建的临时路径） | 高危命令照规则认（第四条末）：纯的判法，拆命令、剥外壳、照字面判目标；工具经 `Tool::hazard` 报给这里。哪一种归哪一类（`Group`、`Kind::group`）、一条命令里认出的每一种（`Assessed`）也在这里（施工 HZ-1） |
| `crates/miyu-session/src/settings.rs` 的 `ConfirmSettings`（施工 HZ-1） | 受保护的命令的五项配置 `permission.confirm.*`，和关掉的那几类（`waived`）；等人确认等多久 `permission.confirm.timeout`（施工 CD-1，交给内核，不进权限策略） |
| `crates/miyu-session/src/default_workspace.rs`（施工 5-14） | 工作目录对应的工作区：太宽的退回这个会话的默认工作区（`miyu_fs::workspace_of`），权限策略、沙盒共用 |
| `crates/miyu-session/src/actor.rs` | 动作「过执行前的链」交给它，当场判（`session/actor.md`） |

### 对外的样子

在 crate 里面，不对外。一个会话一份，造会话、载入时造：工具目录、数据根、系统的家目录（读不出来的是空的）、拒绝时的三句、这台机器上的沙盒能不能用（核心起来时探的，`core.md`；施工 5-4 上）、会话在不在场所里（`Place`，施工 O-31 前：场所是 `local` 的是本机的会话，带着拒绝的那一句；读不出那一句的造不成、载入不了，`shipped text not readable`）。

判一次交进来的：工具名、修正过的参数、这一轮的工作目录、实际生效的权限（级别 `level`，只读开关 `read_only`）。交回的结论（`Verdict`）：

| 结论 | 带什么 |
|---|---|
| `Allow` 放行 | |
| `Deny` 拒绝 | `module`：`permissions`；`text`：写给她的一句；`human`：给人看的说法 |
| `Ask` 要问人 | `module`：`permissions`；`access`：这件工具的访问类别；`rule`：提的放行规则，可以没有；`detail`：给头看的说明 |

实际生效的那一级：

| 那一级 | 什么时候 |
|---|---|
| 完全放开 | 级别是 `full`，只读开关关着（A16 以前的 `workspace` 读成 `full`，施工 LV-1） |
| 只读 | 只读开关开着；或者级别不认识，按最严的算 |

A16 起只剩这两级（`11-权限与沙盒.md` A16，施工 LV-1），代码在 `crates/miyu-session/src/guard/levels.rs`（`Effective`、`mark`）。

边界表的几片（`Zone`）：

| 片 | 是什么 |
|---|---|
| 能读能写（`Writable`） | 工作区、加进来的目录（施工 5-10 上）、临时目录 |
| 只能读（`Readable`） | 系统目录、工具链目录（只禁写的名单、受保护的路径在施工 LV-5 去掉了，`11-权限与沙盒.md` A17：工作区里的 `.git/hooks` 这些照工作区算） |
| 谁都不能碰（`Forbidden`） | Miyu 的数据根 |
| 私密（`Private`） | 别的程序的私密目录：钥匙、登录、浏览器配置、旧版 Miyu 的数据（施工 5-13，清单见 `11-权限与沙盒.md` 第四节）；排在数据根、加进来的目录后面 |
| 单独放开只读（`Shelf`） | 数据根里开着的技能文件夹（施工 X-10 上，`skills.md`「技能」第 3 条）：造会话、载入时照这一刻找到的技能放（`Guard::with_shelves`） |
| 边界以外（`Outside`） | 别的 |

### 怎么走

**1. 判一次调用**

1. 工具目录里没有这件：放行，执行时再报现在用不了（`session/tools.md`）。
2. 问工具这次调用要碰的路径、是读是写（下表）：只看参数，不碰磁盘，她看过的不交给它。参数不对的，工具交回空的，执行时再报错。
3. 一条都没有：照它做什么判（第 4 条）。
4. 这一轮的工作目录也换成真实的位置（头报来的可能是 `~`）；换不成的照原样用。
5. 造边界表（第 2 条）。
6. 照工具报的先后，一条条判：
   1. 换成真实的位置：碰的是这一条本身的（`itself`，现在只有 `trash`），照 `resolve_itself`：上级换成真实的位置，再接上最后一段的原样，链接判的是链接本身在哪；没有名字可碰的（`.`、`..`、`~`），照整条换。别的：相对的照工作目录接上，`~` 自己、`~/` 开头的（Windows 上 `~\` 也算）照家目录接上，`~alice` 不算；链接、Windows 的目录联接照它们指向的地方算；还不存在的，照最近的、已经存在的上级目录算（`fs.md`）。换不成：拒绝，写明是哪一条、为什么，后面的不判了。
   2. 查它落在哪一片。
   3. 照判法表（第 3 条）：拒绝的，当场拒绝，后面的不判了；要问人的，记下；放行的，看下一条。
7. 放行过的不问（施工 D-1）：要问人的里，落在本会话放行过的范围里的去掉。内核交来的 `grants` 是人选过「本会话都允许」的那几条规则（`kernel/asking.md`「本会话放行过的」）；这一条是写的，看规则的 `write`，是读的看 `read`，是其中一个本身、或者在它下面就算放行过。按路径的段比（`/a/b` 不放行 `/a/bc`）；读不懂的规则不算。私密目录里的那一条只认放行范围本身也在私密目录里的规则（施工 5-15：放行了家目录不等于放行了 `~/.ssh`）。拒绝的、只读拦下的在这之前就定了，放行过的也照拒：只读压过放行，数据根哪一级都不许。
8. 都判完了：有要问人的，问人（第 5 条）；没有，放行。
9. 所以拒绝胜过问人，问人胜过放行；拒绝时说的是照先后第一条被拒的那一条。
10. 在阻塞线程里判，actor 等它判完，再把「链判完了」送进内核（`session/actor.md`）：换真实的位置、造边界表都碰磁盘，不占跑异步任务的线程，慢盘上只让这个会话自己等（施工 4-9 再补四下：原来当场在 actor 的异步线程里判）。
11. 第 6 条换成的真实位置（照判的先后，判到拒绝的那一条为止）连同结论一起交回（施工 5-18）：actor 交给执行器记下，派的时候放进 `Call.checked`，工具只碰这几个（`session/tools.md` 第 1 条、`tools/interface.md`「三、要碰的路径」第 4 条）。判的和碰的是同一次换出来的：工具自己再换一次、和判过的对不上，就是判完以后路径上的链接被换过，不碰。

| 工具 | 报的路径 | 读还是写 |
|---|---|---|
| `read` | `file_path` | 读 |
| `glob`、`grep` | 搜的地方（`tools/glob.md`、`tools/grep.md`） | 读 |
| `write`、`edit` | `file_path` | 写 |
| `trash` | `file_path`，碰的是这一条本身 | 写 |
| `shell` | 不报 | |

**2. 边界表**

1. 几片：工作区每判一次照这一轮的工作目录；别的几片造权限策略时（造会话、载入时）照那时的环境读一次，以后照它（施工 4-9 再补四下：原来每判一次重读环境变量）：
   - 工作区：这一轮的工作目录；太宽的（`miyu_fs::too_wide`：根目录、数据根里的，Windows 上还有家目录、包含数据根的，施工 5-14 改）是这个会话的默认工作区（施工 5-14，`default_workspace.rs`；子会话、分叉出来的用 `session.created` 的 `workspace_from` 那一个），用到时建好：边界表不算不存在的那一片。相对路径照旧从工作目录算。
   - 加进来的目录：这一轮的，每一个都照工作区算（施工 5-10 上）。太宽的，协议端点已经拒绝了。
   - 数据根。
   - 临时目录：系统的临时目录。
   - 只能读的：下表的系统目录；家目录下的 `.cargo`、`.rustup`、`.npm`、`.gitconfig`；环境变量 `CARGO_HOME`、`RUSTUP_HOME` 设了、不是空的，它们指到的地方。家目录不知道的，工具链目录只算环境变量指到的。
2. 每一片也换成真实的位置；不存在、换不成的那一片不算。
3. 照这个先后，先对上的算：
   0. 最深的那一片算（施工 5-14 改）：工作区是 `~` 这种包住数据根、私密目录、开着的技能文件夹的，那几处照下面它们自己那一条算（数据根谁都不能碰、私密目录私密、技能文件夹只读）；工作区落在它们里面的才照第 1 条。
   1. 在工作区里：能读能写。
   2. 在数据根里：谁都不能碰。工作区退回到了数据根里的 `root/workspace/` 的，那一片照第 1 条算。
   3. 在加进来的哪一个目录里：照第 1 条（施工 5-10 上）。排在数据根后面：落进了数据根的（报来以后被换成了链接），数据根照样谁都不能碰。
   4. 在临时目录里：能读能写。数据根可能就在临时目录里（测试、`MIYU_HOME` 指到那里），所以数据根排在前面。
   5. 在只能读的哪一片里：只能读。
   6. 别的：边界以外。
4. 在不在里面，照路径一段一段比：`/a/bc` 不在 `/a/b` 里；这一片自己也算在里面。
5. 大小写：macOS、Windows 上，判在不在数据根里不分大小写，免得换个大小写就绕过去；判在不在工作区、临时目录、只能读的那几片里，照原样比：拿不准的落到边界以外，多问一次人，不会多放行。

| 平台 | 系统目录 |
|---|---|
| Linux | `/usr`、`/bin`、`/sbin`、`/lib`、`/lib32`、`/lib64`、`/etc`、`/opt` |
| macOS | `/usr`、`/bin`、`/sbin`、`/System`、`/Library`、`/Applications`、`/opt`、`/etc`（真实的位置在 `/private/etc`，换过就对上了） |
| Windows | `%SystemRoot%`、`%ProgramFiles%`、`%ProgramFiles(x86)%`，没设的、空的不算 |
| 别的 Unix | 一个都没有：要读就问人 |

**3. 判法表**：一条路径，照实际生效的那一级。

| 这一条是 | 完全放开 | 只读 |
|---|---|---|
| 数据根里的读、写 | 拒绝 | 拒绝 |
| 私密目录里的读（施工 5-13） | 放行 | 问人 |
| 私密目录里的写 | 放行 | 拒绝：只读 |
| 放开只读的技能文件夹里的读 | 放行 | 放行 |
| 放开只读的技能文件夹里的写 | 拒绝 | 拒绝 |
| 能读能写那几片里的读 | 放行 | 放行 |
| 能读能写那几片里的写 | 放行 | 拒绝：只读 |
| 只能读那几片里的读 | 放行 | 放行 |
| 只能读那几片里的写 | 放行 | 拒绝：只读 |
| 边界以外的读 | 放行 | 放行 |
| 边界以外的写 | 放行 | 拒绝：只读 |

所以报路径的调用要问人的只剩一种：只读时读私密目录（施工 LV-1 去掉了工作区那一列）。

只读时访问类别要写入的调用，内核在交给链之前就拦下了（`kernel/session.md`）；走到这里的写，也拒绝。

**4. 不报路径的调用**：照工具的访问类别。

| 访问类别 | 完全放开 | 只读 |
|---|---|---|
| `read`、`write` | 放行 | 放行 |
| `execute` | 放行，不进沙盒 | 沙盒能用：放行（在只读沙盒里跑）；用不了：问人 |
| `execute`，工具报高危（施工 5-17） | 问人，不提规则（A16，施工 LV-1）；本人的会话里设置页「受保护的命令」关掉的那几类不问、照上面两行（A18，施工 HZ-1）；灭顶的拒绝 | 不看：照上面两行 |
| `network`、`outbound`、不认识的 | 放行 | 问人 |
| `venue`（施工 O-31 前） | 场所会话：放行；本机的会话：拒绝（`venues` 有 `local` 的照 `outbound`，施工 O-31 补） | 同左 |

- 读写不报路径的放行：查不到路径的，执行时工具自己报错。
- 扩展的工具声明了 `paths` 的（施工 XG-1 上，设计 63 第三节，`providers.md`「声明了 `paths` 的」）照参数报路径，走第三条那张表，和内置 `edit` 一样；没声明的照这一条。
- 在场所里做的事（`venue`，施工 O-31 前，2026-10-10 核心定）不看级别、不看报的路径：场所会话（场所不是 `local` 的）里放行，不问人，有没有人能确认都一样（场所会话本来就没人能确认，问了等于拒）；本机的会话（终端、网页）里拒绝，`by` 是 `permissions`，写给她出厂的那一句（`core/permissions/not-in-venue.txt`），说法 `core/permissions/not-in-venue`。也给本机用的（工具的 `venues` 里有 `local`，施工 O-31 补，2026-10-17 接入QQ 的会话要的「主动发消息」）本机的会话里不拒，照对外发消息（`outbound`）判：完全放开放行，只读时问人，没人能确认的拒（本机的会话就是主人本人）。谁能叫、能动谁是提供者的事（`providers.md`「在场所里做的事」）。本机的会话的工具面本来就没有只给场所的工具（`venues` 不含 `local`），这一条是兜底。
- 只碰得到自己工作区的会话（施工 5-12，`11-权限与沙盒.md` 第三节「外部身份」，2026-10-11 项目主人定）：关在沙盒里的场所会话（群会话、陌生人的私聊；记在 `session.created` 的 `confined`，载入照它）。不看级别：文件类工具报的每一条路径换成真实的位置以后，要在这一轮的工作区或者加进来的目录里，外面的（连读）一律拒（放开只读的技能文件夹里的读除外，施工 X-10 上），`by` 是 `permissions`，写给她出厂的那一句（`core/permissions/outside-workspace.txt`，字段 `path`），说法 `core/permissions/outside-workspace`；访问类别是执行命令的：沙盒能用、又关得住读、系统服务的 Unix 套接字也管得住（`miyu_sandbox::confines_others`：读要 Linux、macOS，套接字在 Linux 上要 Landlock 第 9 版或 seccomp，施工 MB-1）的放行，在只准读名单里的沙盒里跑（施工 5-12 下，`session/tools.md` 第 1a 条）；别的一律拒（`no-commands.txt`，说法 `core/permissions/no-commands`）；装包卸包的（访问类别 `install`）一律拒。主人本人的场所会话、本机的会话照旧。
- 问人时不提规则，只能选允许这一次或者拒绝；说明照第五条第 4 款。
- 越过沙盒问人（`shell` 的 `outside_sandbox`、「此目录不再询问」和它带的命令前缀，施工 D-4、5-15、SB-3）、你在配置里事先放行的工具（`sandbox.tools`，施工 X-36、X-37）随「工作区」一级去掉了（`11-权限与沙盒.md` A16，施工 LV-1、LV-2）：命令只在只读时进沙盒，完全放开不进，没有「这一次到沙盒外跑」这回事。
- 高危命令（工具报 `Tool::hazard`，`shell` 照命令的字面认，施工 5-17，`11-权限与沙盒.md` 第四节）：完全放开下管（A16，施工 LV-1，2026-11-09 项目主人定：不做沙盒不等于不做保护；改掉 5-17 再补的「完全放开只拦灭顶的」），判在报的路径之后，先于别的放行（放行过的规则也管不到它）；只读不看，命令本来就在只读沙盒里跑。报的路径已经判了拒绝的照拒，不改成问人。
  - 灭顶的（删根、家目录、根下的系统目录、数据根，格式化、写裸磁盘，容器把整个宿主交出去）拒绝，人点了也不跑：`by` 是 `permissions`，写给她工具交的那一句（`shell` 的 `hazard.txt` 填上 `hazard/<种类>.txt`），说法是工具交的（`shell/hazard-<种类>`）。
  - 别的问人，不提规则（只能允许这一次或者拒绝），没人能确认的拒；说明照第五条第 4 款再多 `hazard`：种类的写法，例如 `{"command":"git push --force","hazard":"git_force_push","sandbox":false,"title":"Push","tool":"shell"}`。
  - 种类：`delete_root`、`delete_home`、`delete_system`、`delete_data_root`、`disk_format`、`disk_write`、`container_host`（施工 5-17 三补）拒绝；`delete_working_dir`、`delete_unresolved`、`git_force_push`、`git_reset_hard`、`git_clean`、`git_discard`、`git_stash_drop`、`power`、`pipe_to_shell`、`container_share`（施工 5-17 三补）问人。
  - 受保护的命令按类给开关（A18，施工 HZ-1，2026-11-11 项目主人定）：问人的十种分五类（`miyu_policy::hazard::Group`），一类一项配置 `permission.confirm.<类>`（`config.md`），出厂都开。工具把一条命令里认出的每一种问人的连同归的类全报上来（`Hazard::Asks`，一种一个 `HazardAsk`：`code`、`class`），权限策略去掉归关掉的那几类的：还有剩的照剩下的最先认出的那一种问（`hazard` 写它）；一种都不剩的照没认出高危接着判（完全放开就是放行，直接跑）。所以一条命令命中几类的，有一类还开着就照旧问。

    | 类（`permission.confirm.<类>`） | 管哪几种 |
    |---|---|
    | `git` | `git_force_push`、`git_reset_hard`、`git_clean`、`git_discard`、`git_stash_drop` |
    | `delete` | `delete_working_dir`、`delete_unresolved` |
    | `power` | `power` |
    | `pipe_to_shell` | `pipe_to_shell` |
    | `container` | `container_share` |

  - 开关管不到的：灭顶的那七种（没有归的类，照拒）；看不透的命令（命令替换、`eval` 套了 8 层以上，`Kind::TooDeep`）不归哪一类（`class` 是空的），五个开关全关也照旧问，对外的写法照旧是 `delete_unresolved`；报的路径已经拒了的；只读（不看高危）；装包卸包（访问类别 `install`，不走高危这条路，完全放开每次问，这一步没给开关）。
  - 只有本人的会话看开关（本机的头、主人自己在场所里的会话）。只碰得到自己工作区的会话（`confined`）不看，照旧都问（这些会话没人能确认，问了等于拒）。
  - 开关的值照回合开始时的配置（同私密目录）：`actor/model.rs` 的 `turn_start` 读 `ConfirmSettings`，把关掉的那几类换进权限策略（`Guard::set_waived`），判的时候不读配置；改了下一轮生效。会话刚造好、还没开过回合时是空的（都问）。
- 访问类别是 `write` 或者不认识的，内核当成写入：只读时在交给链之前就拦下了，走不到这里（`kernel/session.md`）。

**5. 问人时提的规则和说明**

1. 放行规则：哪件工具（`tool`）；要问人的那几条，照是读是写（`read`、`write`），各列出放行的范围（A16 起要问人的只剩只读时读私密目录，现在只会有 `read`，施工 LV-1）：是目录的就是它自己，别的（文件、还不存在的）是它的上一级；这个目录是家目录本身、或者更上面的，换成这一条本身：家目录里有钥匙（施工 D-1，`11-权限与沙盒.md` 第二节「放行规则管多大」）。家目录也换成真实的位置再比：不换的话 Windows 上一边带 `\\?\` 前缀、一边不带，家目录在链接后面时也一样，永远比不上（施工 D-1，CI run 960）。照字排好、去掉重复的；哪一种一条都没有，就不写那一格。本会话都允许、这个工作区以后都允许时，放行的就是这些目录（`kernel/asking.md`）。
2. 给头看的说明：哪件工具；每一条要问人的路径，照判的先后：真实的位置、是不是写、在哪一片（`private` 是别的程序的私密目录，施工 5-13；A16 起要问人的只有只读时读私密目录，别的片写 `outside`）。
3. 都写成紧凑的 JSON，键照字母排；路径写成字，不是 UTF-8 的地方换成 `�`。
4. 不报路径的调用的说明（施工 D-4，D-1 补）：`{"tool": <工具名>}`，并进工具交的几格（`Tool::asking`，`shell` 交 `title`、`command`）；执行类的另写 `sandbox`：这一次在不在沙盒里跑；走到问人的执行不在沙盒里（完全放开不进沙盒，只读时是沙盒用不了），都是 `false`。例如 `{"command":"touch ~/Downloads/x.txt","sandbox":false,"title":"Create a file","tool":"shell"}`。

在工作区 `/home/alice/src` 里、只读开着，`read` 读 `~/.ssh/config`（私密目录）时的规则、说明：

```json
{"read":["/home/alice/.ssh"],"tool":"read"}
{"paths":[{"path":"/home/alice/.ssh/config","write":false,"zone":"private"}],"tool":"read"}
```

**6. 拒绝时写给她的**

| 什么时候 | 写给她的一句 | 说法 |
|---|---|---|
| 碰到数据根 | `"{path}" is inside Miyu's own data, which no tool can read or change.` | `core/permissions/forbidden`，字段 `path` |
| 换不成真实的位置 | `Can't tell where "{path}" points: {reason}.` | `core/permissions/unresolvable`，字段 `path`、`reason` |
| 只读的时候要写 | `Not run: the session is read-only.` | `core/tool-results/read-only` |

- 三句的原文行尾都带换行；造会话、载入时拿空的字段试换过一次，要了不该有的字段的，会话造不成、载入不了（`policy.md`）。
- `path` 是工具报的原样，不是换过的真实位置。换进去的字段照模板转义（`kernel/request.md`）；说法里的字段是原样的。
- `reason` 是换不成的原因，英文：

| 为什么换不成 | `reason` |
|---|---|
| `~` 自己、`~/` 开头，可家目录不知道 | `the home directory is not known` |
| 还不存在的那几段里有 `..`（Windows 上照字面消掉 `..`，不报） | `'..' after a directory that does not exist` |
| 路径上有一个链接，指向的地方不存在 | `a link on the path points nowhere` |
| 连根都找不到存在的上级目录，或者读上级目录时出了错 | 出错的原话 |

**7. 判完以后**（内核怎么走，`kernel/asking.md`）

| 结论 | 内核 |
|---|---|
| 放行 | 派去执行 |
| 拒绝 | 记一条被拒绝的结果（`denied`），`by` 是模块 `permissions`，带着写给她的一句和说法；这一步照常往下走，她接着干 |
| 要问人，会话里没人能确认（例如 `miyu ask`） | 当场拒绝，`by` 是内核，写给她的是 `Not run: it needs the user's approval, and no one here can give it.`，说法 `core/tool-results/unattended` |
| 要问人，只读时访问类别要写入 | 当场拒绝，`by` 是内核，写给她的是只读那一句 |
| 要问人，别的 | 记一条 `tool.approval_requested`，`by` 是模块 `permissions`，照写 `access`、`rule`、`detail`，这次调用停下来等人；等到配置的时限（`permission.confirm.timeout`，出厂 5 分钟）还没人确认，内核替人拒绝，写给她的是 `Not run: no one answered the approval request in time.`（施工 CD-1，`kernel/asking.md`「到时没人确认」） |

### 给人看的字

| 说法 | 中文 | 英文 |
|---|---|---|
| `core/permissions/forbidden` | 这是 Miyu 自己的数据，谁都不能碰 | Miyu's own data, off limits |
| `core/permissions/unresolvable` | 说不清它指向哪里：{reason} | can't tell where it points: {reason} |
| `core/tool-results/read-only` | 只读模式，未执行 | Read-only mode, not run |
| `core/permissions/not-in-venue`（施工 O-31 前） | 只能在群聊、私聊里用 | Only works in a group or private chat |
| `core/permissions/outside-workspace`（施工 5-12） | 工作区外，未执行 | Outside the workspace, not run |
| `core/permissions/no-commands`（施工 5-12） | 这里不能执行命令 | Commands are not available here |

字在 `resources/core/human/{zh,en}.json`（`store/resources.md`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-session/src/guard/tests.rs` | 判法表的每一格（边界以外的读哪一级都放行）；实际生效的那一级，不认识的按只读，以前的 `workspace` 读成完全放开（施工 LV-1）；不报路径的：执行命令沙盒能用时两级都放行，用不了时完全放开照样放行、只读问人、不提规则、说明是 `{"sandbox":false,"tool":"shell"}`（施工 D-4 起），读写放行，联网只读问人、完全放开放行（施工 5-4 上）；说明并进工具交的几格、工具名盖不过、执行类才写 `sandbox`，沙盒用不了时也带标题和命令（施工 D-4） |
| `crates/miyu-session/tests/hazard.rs`（施工 5-17） | 真的 `shell`：完全放开删家目录拒、不问，结果是拒绝、带那一句；`git push --force`、`git stash drop` 问、不提规则、说明带 `hazard`、`command`，没人能确认的拒；只读不看；平常的命令完全放开照旧不问。`permission.confirm.git` 关着的那一轮 `git reset --hard` 不问就跑，改回开着下一轮又问（施工 HZ-1） |
| `crates/miyu-session/src/guard/tests.rs` 的 `a_hazard_never_softens_a_refusal`（施工 5-17） | 报的路径已经拒了的照拒（那一类关掉了也照拒）；灭顶的拒、拿工具交的那一句；别的问人、不提规则、说明带种类 |
| `crates/miyu-session/src/guard/tests/protected.rs`（施工 HZ-1） | 真的 `shell` 报高危，直接调权限策略：没关的每一种都问；关一类，这一类的放行、别的类照旧问；直接拒的七种五类全关也照拒、和没关时一样；命中两类的只关一类照旧问、问的是还开着的那一种，两类都关才放行；九层命令替换里的删除、十层 `eval` 五类全关也照旧问、说明是 `delete_unresolved`；外部身份的会话五类全关每一条判下来的和没关时一样、没有一条放行；只读五类全关和没关时一样 |
| `crates/miyu-session/src/settings/tests.rs` 的 `each_protected_group_has_a_switch_that_is_on_by_default`（施工 HZ-1） | 五项的键照 `Group::ALL` 一类一项，出厂开、只有系统一层、下一轮生效、在「隐私与安全」页「受保护的命令」组；关掉的那几类照类的写法交出去 |
| `crates/miyu-policy/src/hazard/tests.rs`、`hazard/lex/tests.rs`（施工 5-17） | 三类各几条认得出、拒的和问的分对；连写、外壳、`sh -c`、`eval`、替换里的认得出；平常的删、git 不误报；拆命令的边界、引号、反斜杠、注释、替换、重定向（读进来的、文件描述符不是参数） |
| `crates/miyu-policy/src/hazard/tests/fresh.rs`（施工 HZ-2） | `x=$(mktemp …)` 以后删 `$x`、`"$x"`、`${x}`、`"${x}"` 不问，几种 `mktemp` 写法、反引号、`-u` 都认；带后缀的、赋值前后拼了字或里面不止一条 `mktemp`、参数带变量的、中间改过的（赋值、`read`、`for`、`printf -v`、`declare -n`、`${x:=…}`、算术）、赋值不一定跑到的（分支、`&&`、`\|\|`、括号、花括号、`&`、`case`、管道）、循环和函数里改的、有 `eval`、`source`、`trap`、`IFS` 的、跨命令的、`sh -c` 和替换里的、`find`、别的命令替换照旧判；同一个 `rm` 里别的目标照原规矩、最严者胜；`$(echo /)x` 不当 `$x` |
| `crates/miyu-policy/src/hazard/tests/groups.rs`（施工 HZ-1） | 问人的十种正好分进五类，直接拒的七种和看不透的不归类；套得太深的单列一种、对外的写法是 `delete_unresolved`，八层还看得透，目标运行时才知道的删除照旧归「递归删除」；一条命令里认出几种问人的全交回、照先后、不重复，有拒的只交回拒的那一种 |
| `crates/miyu-session/tests/guard.rs` | 相对路径照工作目录接、工作区里还不存在的也能写；越界的读哪一级都放行（施工 5-4 上）；数据根哪一级都拒、写对了那一句和说法；完全放开越界的写不问；执行命令（施工 LV-1）：完全放开沙盒能不能用都照跑，只读沙盒能用不问、用不了问人（没人能确认就拒）；几条路径照最严的，有一条在数据根里就拒、不问；指向不存在处的链接拒绝、写对了那一句和说法；`~` 照家目录换，完全放开照写、工具拿到的判过的位置是家目录下那一个；`trash` 判的是链接本身：工作区里指向不存在处的、指进数据根的、指到外面的链接都放行 |
| `crates/miyu-session/src/guard/tests.rs` 的 `a_venue_action_runs_only_in_a_venue`、`crates/miyu-session/tests/guard_venue.rs`（施工 O-31 前） | 在场所里做的事：场所会话（群、私聊）里放行、不问人，只读开着、没人能确认也一样；本机的会话拒绝，`by` 是 `permissions`，写给她出厂的那一句、说法对，工具没跑；载入以后照样；`venues` 有 `local` 的在本机的会话里照 `outbound`：完全放开跑，只读没人能确认的拒、说的不是「不在场所里」（施工 O-31 补） |
| `crates/miyu-endpoint/tests/provide_venue.rs`（施工 O-31 前，O-31 补改） | 契约：扩展登记 `venue` 的工具收；`venues` 有 `local` 的本机的会话调到它照 `outbound` 判，只读时问人，人点头以前扩展收不到 `tool.call`（完全放开直接发，施工 LV-1）。扩展登记的工具 `venues` 没有 `local` 的本来就不在本机会话的工具面上，「不在场所里」那一句只剩给没写 `venues` 的工具兜底（`guard_venue.rs`） |
| `crates/miyu-session/tests/guard_confined.rs`、`guard_venue.rs`、`src/sandbox/tests.rs`（施工 Y-2 下①、SU-4） | 关起来的会话（群）：工作区里读得到，别处拒，命令拒；载入以后、分叉出来的照样关着；装不了包；主人本人的私聊、本机的会话不关 |
| `crates/miyu-session/tests/sandbox_member.rs`（施工 MB-1） | 群会话真经助手跑 `shell`：读得到自己的工作区、`/etc/hosts`、`git`；读不到数据根、跑核心的用户的家目录、别处、`/etc` 名单外的、`/opt`；要出沙盒的拒、不问，拒的话里没有核心那边的绝对路径；主人在群里说话照样关着；关不住读的平台上不跑 |
| `crates/miyu-session/tests/sockets_off.rs`（施工 MB-1，自己一个进程） | 套接字管不住的机器上群会话不跑命令，本机的会话照跑；管得住以后照跑 |
| `crates/miyu-session/tests/guard_grants.rs`（施工 D-1） | 只读开局读私密目录（A16 起文件工具问人的只剩这一种，施工 LV-1）：选了本会话都允许，同一个目录下面的读不再问、名字只是开头一样的旁边的目录照旧问；家目录里的一个文件只放它本身、同在家目录里的别的照旧问；家目录在链接后面也照真实的位置比（只在 Unix 上） |
| `crates/miyu-session/tests/guard_checked.rs`（施工 5-18） | 判过的真实位置交到工具手里：报的几条照先后（相对的、外面的、还不存在的），不报路径的是空的；脚本里调的一样带；工作区里的链接在问人时被换成指进数据根，人点了允许，`write` 不写、说路径在判过以后变了（只在 Unix 上） |
| `crates/miyu-session/tests/guard_dirs.rs` | 加进来的目录（施工 5-10 上）：里面写不用问、只读照旧拒绝 |
| `crates/miyu-session/tests/guard_private.rs`（施工 5-13） | 私密目录：只读时读问人，说明里是 `private`、规则放行整个目录；没人能确认的不跑；只读的写不问直接拒；完全放开读写都不问；工作区、加进来的目录在里面的照人给的算，只读时读里面的不问（施工 LV-1） |
| `crates/miyu-session/tests/guard_protected.rs`（施工 LV-5） | 受保护的路径撤了以后：工作区里的 `.git/hooks`、`.git/config`、`.vscode`、`Makefile` 和旁边平常的文件判的一样（完全放开读写都不问，只读读放行、写拒）；老配置里的 `sandbox.readonly.*` 不起作用 |
| `crates/miyu-fs/tests/boundary.rs` | 工作区能读能写、`.git/hooks` 这些和工作区里别的一样（只禁写的名单施工 LV-5 撤了）；数据根在临时目录里也不能碰；工作区退回到数据根里照样是工作区；临时目录能读能写、系统和工具链只能读、别的在边界以外；不存在的一片不算；大小写不分的平台上数据根不分大小写；这一片自己也算；这台机器的系统目录、临时目录 |
| `crates/miyu-fs/tests/resolve.rs` | 相对路径、`~`、还不存在的照上级、链接照指向的地方、Windows 的两种分隔符和目录联接 |

### 出处

- `11-权限与沙盒.md` 第二节：权限级别（A16 起只剩完全放开一级，施工 LV-1）、只读是叠在上面的开关、只读时沙盒能用执行命令不问、用不了时问人、「第一版的权限策略怎么判」（判法表、换不成就拒、放行规则、给头看的说明）；A2、A12、A13、A16。
- `11-权限与沙盒.md` 第四节：边界、几片重叠时谁说了算、第一版的清单、`.git` 的钩子和配置只读、当前目录太宽；A4。
- `11-权限与沙盒.md` 第七节、A9：核心进程里的文件工具由内核照同一份边界查路径；A11：没有确认界面的场所，要确认的一律拒绝。
- `05-内核接口.md` 第五节第 1 条：工具执行前的链，最严者胜；第六节：要碰的路径。
- `02-内核.md` 第六节「确认怎么走」：放行、拒绝、要问人以后内核怎么走。

### 还没有的

- 工具链目录的缓存可以写（`11-权限与沙盒.md` 第四节的表）：现在这几片对文件工具只能读，写要问人。
- 链上别的守卫，守卫超时按拒绝算（`05-内核接口.md` 第五节第 3 条）。
- 边界是策略数据、配置那一步能改（`11-权限与沙盒.md` 第四节）：现在清单写在代码里。
