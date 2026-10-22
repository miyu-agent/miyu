## 权限策略

### 是什么

执行前那条链的默认实现，模块编号 `permissions`。工具报出这次调用要碰的路径，换成真实的位置、查边界表，每一条照实际生效的那一级判，合起来照最严的：有一条拒绝就拒绝，有一条要问人就问人。不报路径的调用，照它做什么判：执行命令，沙盒能用就放行、在沙盒里跑（执行器写规格，`session/tools.md`），用不了的问人（施工 5-4 上）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-session/src/guard.rs` | 判一次调用：实际生效的那一级、判法表、不报路径的、问人时提的规则和说明、拒绝 |
| `crates/miyu-fs/src/resolve.rs` | 换成真实的位置，`~` 接家目录（`fs.md`） |
| `crates/miyu-fs/src/boundary.rs` | 边界表：一个真实的位置落在哪一片（`fs.md`） |
| `crates/miyu-policy/src/guard.rs` | 拒绝时写给她的三句，造会话、载入时从策略快照里拿 |
| `resources/core/permissions/forbidden.txt`、`unresolvable.txt`，`resources/core/tool-results/read-only.txt` | 三句的原文 |
| `resources/core/permissions/not-in-venue.txt`（施工 O-31 前） | 本机的会话里拒绝在场所里做的事的那一句：不进策略快照，本机的会话造、载入时读（`crates/miyu-session/src/open.rs` 的 `place`） |
| `crates/miyu-session/src/guard/untargeted.rs` | 不报路径的调用（第四条）：照访问类别、在场所里做的事、要在沙盒外跑的（施工 5-18 从 `guard.rs` 挪出来：那边到了 500 行） |
| `crates/miyu-session/src/guard/asking.rs` | 要问人的那几条：提的放行规则、给头看的说明、放行过的管不管得到（施工 X-10 上从 `guard.rs` 挪出来） |
| `crates/miyu-session/src/guard/confined.rs`、`resources/core/permissions/outside-workspace.txt`、`no-commands.txt`（施工 5-12） | 只碰得到自己工作区的会话（第四条末）：判在不在工作区、拒绝时的两句；两句同样不进策略快照，这种会话造、载入时读 |
| `crates/miyu-policy/src/hazard.rs`、`hazard/lex.rs`、`hazard/rules.rs`（施工 5-17）、`hazard/vars.rs`（施工 5-17 再补：同一条命令里赋过值的变量） | 高危命令照规则认（第四条末）：纯的判法，拆命令、剥外壳、照字面判目标；工具经 `Tool::hazard` 报给这里 |
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
| 完全放开 | 级别是 `full`，只读开关关着 |
| 工作区 | 级别是 `workspace`，只读开关关着 |
| 只读 | 只读开关开着；或者级别不认识，按最严的算 |

边界表的几片（`Zone`）：

| 片 | 是什么 |
|---|---|
| 能读能写（`Writable`） | 工作区、加进来的目录（施工 5-10 上）、临时目录 |
| 只能读（`Readable`） | 系统目录、工具链目录；能写的那几片里落在只禁写名单（受保护的路径）上的：工作区里的 `.git/hooks`、`.git/config`、编辑器和 agent 的配置，工作区是家目录时家目录下的 shell 启动文件这些（施工 5-17 下，`fs.md` 第一节第 6 条） |
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
7. 放行过的不问（施工 D-1）：要问人的里，落在本会话放行过的范围里的去掉。内核交来的 `grants` 是人选过「本会话都允许」的那几条规则（`kernel/asking.md`「本会话放行过的」）；这一条是写的，看规则的 `write`，是读的看 `read`，是其中一个本身、或者在它下面就算放行过。按路径的段比（`/a/b` 不放行 `/a/bc`）；读不懂的规则不算。私密目录里的那一条只认放行范围本身也在私密目录里的规则（施工 5-15：放行了家目录不等于放行了 `~/.ssh`）。受保护的路径（只禁写，施工 5-17 下，2026-10-22 定，同 Claude Code 的危险文件）同私密目录：只认放行范围本身也是受保护路径的规则（`Boundary::protected`），对整个工作区点了「此目录不再询问」，里面的 `.git/hooks` 照旧问。问它时提的规则：它是文件、上级又不受保护的（工作区里的 `.mcp.json`），只放这一条本身（`Asked::alone`），不然放它的上级盖不住它；上级也受保护的（`.git/hooks/pre-commit`）照旧放上级。拒绝的、只读拦下的在这之前就定了，放行过的也照拒：只读压过放行，数据根哪一级都不许。
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
   - 只禁写的名单（施工 5-17 下）：造的时候是出厂的，回合开始照这一轮的配置换（`sandbox.readonly.add`、`sandbox.readonly.remove`，`Guard::readonly_of`、`Guard::set_readonly`，同私密目录），和在跑的调用共用，所以上锁。
2. 每一片也换成真实的位置；不存在、换不成的那一片不算。
3. 照这个先后，先对上的算：
   0. 最深的那一片算（施工 5-14 改）：工作区是 `~` 这种包住数据根、私密目录、开着的技能文件夹的，那几处照下面它们自己那一条算（数据根谁都不能碰、私密目录私密、技能文件夹只读）；工作区落在它们里面的才照第 1 条。
   1. 在工作区里：落在只禁写名单上的只能读（施工 5-17 下：相对的那几项哪一层接得上都算，原来写死的 `.git/hooks`、`.git/config` 是出厂的头两项；沙盒外执行 git、打开编辑器、进目录时会跑它们）。别的能读能写。
   2. 在数据根里：谁都不能碰。工作区退回到了数据根里的 `home/<账号>/workspace/` 的，那一片照第 1 条算。
   3. 在加进来的哪一个目录里：照第 1 条（施工 5-10 上）。排在数据根后面：落进了数据根的（报来以后被换成了链接），数据根照样谁都不能碰。
   4. 在临时目录里：能读能写，落在只禁写名单上的只能读。数据根可能就在临时目录里（测试、`MIYU_HOME` 指到那里），所以数据根排在前面。
   5. 在只能读的哪一片里：只能读。
   6. 别的：边界以外。
4. 在不在里面，照路径一段一段比：`/a/bc` 不在 `/a/b` 里；这一片自己也算在里面。
5. 大小写：macOS、Windows 上，判在不在数据根里、判在不在只禁写名单上不分大小写，免得换个大小写就绕过去；判在不在工作区、临时目录、只能读的那几片里，照原样比：拿不准的落到边界以外，多问一次人，不会多放行。

| 平台 | 系统目录 |
|---|---|
| Linux | `/usr`、`/bin`、`/sbin`、`/lib`、`/lib32`、`/lib64`、`/etc`、`/opt` |
| macOS | `/usr`、`/bin`、`/sbin`、`/System`、`/Library`、`/Applications`、`/opt`、`/etc`（真实的位置在 `/private/etc`，换过就对上了） |
| Windows | `%SystemRoot%`、`%ProgramFiles%`、`%ProgramFiles(x86)%`，没设的、空的不算 |
| 别的 Unix | 一个都没有：要读就问人 |

**3. 判法表**：一条路径，照实际生效的那一级。

| 这一条是 | 完全放开 | 工作区 | 只读 |
|---|---|---|---|
| 数据根里的读、写 | 拒绝 | 拒绝 | 拒绝 |
| 私密目录里的读（施工 5-13） | 放行 | 问人 | 问人 |
| 私密目录里的写 | 放行 | 问人 | 拒绝：只读 |
| 放开只读的技能文件夹里的读 | 放行 | 放行 | 放行 |
| 放开只读的技能文件夹里的写 | 拒绝 | 拒绝 | 拒绝 |
| 能读能写那几片里的读 | 放行 | 放行 | 放行 |
| 能读能写那几片里的写 | 放行 | 放行 | 拒绝：只读 |
| 只能读那几片里的读 | 放行 | 放行 | 放行 |
| 只能读那几片里的写 | 放行 | 问人 | 拒绝：只读 |
| 边界以外的读 | 放行 | 放行 | 放行 |
| 边界以外的写 | 放行 | 问人 | 拒绝：只读 |

只读时访问类别要写入的调用，内核在交给链之前就拦下了（`kernel/session.md`）；走到这里的写，也拒绝。

**4. 不报路径的调用**：照工具的访问类别。

| 访问类别 | 完全放开 | 工作区 | 只读 |
|---|---|---|---|
| `read`、`write` | 放行 | 放行 | 放行 |
| `execute` | 放行 | 沙盒能用：放行（在沙盒里跑）；用不了：问人 | 沙盒能用：放行（在只读沙盒里跑）；用不了：问人 |
| `execute`，要在沙盒外跑（施工 D-4） | 放行 | 问人，提「此目录不再询问」：放行工作目录的写（施工 5-15） | 拒绝：只读 |
| 管理员在系统配置里放行了的（`sandbox.tools.<名字> = "<来处>"`，施工 X-36，`11-权限与沙盒.md` A15） | 放行，不带沙盒 | 放行，不问、不带沙盒 | 拒绝：只读 |
| `execute`，工具报高危（施工 5-17） | 不问，照上面两行；灭顶的拒绝（施工 5-17 再补） | 问人，不提规则；灭顶的拒绝 | 不看：照上面两行 |
| `network`、`outbound`、不认识的 | 放行 | 问人 | 问人 |
| `venue`（施工 O-31 前） | 场所会话：放行；本机的会话：拒绝（`venues` 有 `local` 的照 `outbound`，施工 O-31 补） | 同左 | 同左 |

- 读写不报路径的放行：查不到路径的，执行时工具自己报错。
- 在场所里做的事（`venue`，施工 O-31 前，2026-10-10 核心定）不看级别、不看报的路径：场所会话（场所不是 `local` 的）里放行，不问人，有没有人能确认都一样（场所会话本来就没人能确认，问了等于拒）；本机的会话（终端、网页）里拒绝，`by` 是 `permissions`，写给她出厂的那一句（`core/permissions/not-in-venue.txt`），说法 `core/permissions/not-in-venue`。也给本机用的（工具的 `venues` 里有 `local`，施工 O-31 补，2026-10-17 接入QQ 的会话要的「主动发消息」）本机的会话里不拒，照对外发消息（`outbound`）判：完全放开放行，工作区问人，没人能确认的拒（本机的会话就是管理员本人）。谁能叫、能动谁是提供者的事（`providers.md`「在场所里做的事」）。本机的会话的工具面本来就没有只给场所的工具（`venues` 不含 `local`），这一条是兜底。
- 只碰得到自己工作区的会话（施工 5-12，`11-权限与沙盒.md` 第三节「外部身份」，2026-10-11 项目主人定）：场所会话、属主不是管理员的（群会话、陌生人和成员的私聊；协议端点造、载入时照属主填 `owner_is_admin`）。不看级别：文件类工具报的每一条路径换成真实的位置以后，要在这一轮的工作区或者加进来的目录里，外面的（连读）一律拒（放开只读的技能文件夹里的读除外，施工 X-10 上），`by` 是 `permissions`，写给她出厂的那一句（`core/permissions/outside-workspace.txt`，字段 `path`），说法 `core/permissions/outside-workspace`；访问类别是执行命令的：沙盒能用、又关得住读（`miyu_sandbox::CONFINES_READS`，现在只有 Linux）的放行，在只准读名单里的沙盒里跑（施工 5-12 下，`session/tools.md` 第 1a 条）；别的一律拒（`no-commands.txt`，说法 `core/permissions/no-commands`）；要在沙盒外跑的一律拒。管理员本人的场所会话、本机的会话照旧。
- 问人时不提规则，只能选允许这一次或者拒绝；说明照第五条第 4 款。
- 要在沙盒外跑的执行（工具报 `Tool::outside_sandbox`，`shell` 写了 `outside_sandbox`，施工 D-4）：完全放开放行；工作区问人，不管沙盒能不能用，提的规则是「此目录不再询问」：`{"tool", "write": [工作目录的真实位置]}`，家目录本身也照放（施工 5-15，2026-10-20 项目主人定；原来不提规则）。选了的，这一次照「允许这一次」在沙盒外跑，以后的命令照旧在沙盒里跑、这个目录能写（`session/tools.md`），文件工具写它不再问；越过沙盒的照样每次问（规则只管路径，不放开整个沙盒）；只读拒绝，说只读开着、命令不能在沙盒外跑。本会话放行过的规则管不到它：规则只管路径。执行器照同一个报不写沙盒的规格，所以工作区一定问过人才跑得到。
- 管理员放行了的工具（施工 X-36，`11-权限与沙盒.md` A15，2026-10-20 项目主人定）：系统配置的 `sandbox.tools.<名字>` 写的来处，和工具目录里这件工具这时归的包编号（`Catalog::package_of`）一样，才算放行；回合开始照这一轮的配置把放行表交给策略（同技能文件夹）。放行了的照要在沙盒外跑的判（只碰得到自己工作区的会话拒、只读拒），但工作区这一级不问；执行器不给沙盒的规格。高危的照下一条问，没人能确认的地方照拒。嵌套调用（`miyu call`）走同一条链。工具自己报不报 `outside_sandbox` 都一样：放行只认管理员写的。带文件的工具（脚本）照文件判那一刻的真实位置（施工 X-37 补，`Tool::file`、`unsandboxed::Confines`、`Grant`）：落在家目录自己放东西的几处、资源目录下面才算，落在她这一轮写得了的地方（边界表的 `Writable`、放行过的写的规则）的不算；执行器照沙盒规格的 `write` 列表再比一遍（`Wrap::BareUnlessWritable`），两边一样严。`miyu check`（`check/unsandboxed.rs`）每一条照现在的工具目录查，都是警告：没有这件工具的 `sandbox_tool_missing`，来处对不上的 `sandbox_tool_moved`（印出现在的来处），文件不在管理员管得住的地方的 `sandbox_tool_linked`（印出真实位置），这三种不算放行；放行了 `shell` 的 `sandbox_shell` 照算，提醒一声。
- 高危命令（工具报 `Tool::hazard`，`shell` 照命令的字面认，施工 5-17，`11-权限与沙盒.md` 第四节）：工作区、完全放开两级都管（完全放开只拦灭顶的、问人的那几种照常放行，施工 5-17 再补，2026-10-22 项目主人定），判在报的路径之后、要在沙盒外跑之前，先于别的放行（放行过的规则也管不到它）；只读不看，命令本来就在只读沙盒里跑。报的路径已经判了拒绝的照拒，不改成问人。灭顶的（删根、家目录、根下的系统目录、数据根，格式化、写裸磁盘）拒绝，人点了也不跑：`by` 是 `permissions`，写给她工具交的那一句（`shell` 的 `hazard.txt` 填上 `hazard/<种类>.txt`），说法是工具交的（`shell/hazard-<种类>`）。别的在工作区问人，不提规则（只能允许这一次或者拒绝），说明照第五条第 4 款再多 `hazard`：种类的写法，例如 `{"command":"git push --force","hazard":"git_force_push","sandbox":true,"title":"Push","tool":"shell"}`。种类：`delete_root`、`delete_home`、`delete_system`、`delete_data_root`、`disk_format`、`disk_write`、`container_host`（施工 5-17 三补）拒绝；`delete_working_dir`、`delete_unresolved`、`git_force_push`、`git_reset_hard`、`git_clean`、`git_discard`、`git_stash_drop`、`power`、`pipe_to_shell`、`container_share`（施工 5-17 三补）问人。
- 访问类别是 `write` 或者不认识的，内核当成写入：只读时在交给链之前就拦下了，走不到这里（`kernel/session.md`）。

**5. 问人时提的规则和说明**

1. 放行规则：哪件工具（`tool`）；要问人的那几条，照是读是写（`read`、`write`），各列出放行的范围（施工 5-4 上起读都不问人，现在只会有 `write`）：是目录的就是它自己，别的（文件、还不存在的）是它的上一级；这个目录是家目录本身、或者更上面的，换成这一条本身：家目录里有钥匙（施工 D-1，`11-权限与沙盒.md` 第二节「放行规则管多大」）。家目录也换成真实的位置再比：不换的话 Windows 上一边带 `\\?\` 前缀、一边不带，家目录在链接后面时也一样，永远比不上（施工 D-1，CI run 960）。照字排好、去掉重复的；哪一种一条都没有，就不写那一格。本会话都允许、这个工作区以后都允许时，放行的就是这些目录（`kernel/asking.md`）。
2. 给头看的说明：哪件工具；每一条要问人的路径，照判的先后：真实的位置、是不是写、在哪一片（`outside` 是边界以外，`read_only` 是只能读，`private` 是别的程序的私密目录，施工 5-13）。
3. 都写成紧凑的 JSON，键照字母排；路径写成字，不是 UTF-8 的地方换成 `�`。
4. 不报路径的调用的说明（施工 D-4，D-1 补）：`{"tool": <工具名>}`，并进工具交的几格（`Tool::asking`，`shell` 交 `title`、`command`）；执行类的另写 `sandbox`：这一次在不在沙盒里跑，要在沙盒外跑的、沙盒用不了的都是 `false`。例如 `{"command":"touch ~/Downloads/x.txt","sandbox":false,"title":"Create a file","tool":"shell"}`。

在工作区 `/home/alice/src` 里，`write` 写 `~/notes/a.md`（边界以外）时的规则、说明：

```json
{"tool":"write","write":["/home/alice/notes"]}
{"paths":[{"path":"/home/alice/notes/a.md","write":true,"zone":"outside"}],"tool":"write"}
```

`edit` 改 `.git/hooks/pre-commit`（只能读）时：

```json
{"tool":"edit","write":["/home/alice/src/.git/hooks"]}
{"paths":[{"path":"/home/alice/src/.git/hooks/pre-commit","write":true,"zone":"read_only"}],"tool":"edit"}
```

**6. 拒绝时写给她的**

| 什么时候 | 写给她的一句 | 说法 |
|---|---|---|
| 碰到数据根 | `"{path}" is inside Miyu's own data, which no tool can read or change.` | `core/permissions/forbidden`，字段 `path` |
| 换不成真实的位置 | `Can't tell where "{path}" points: {reason}.` | `core/permissions/unresolvable`，字段 `path`、`reason` |
| 只读的时候要写 | `The call was not run: the session is read-only.` | `core/tool-results/read-only` |

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
| 要问人，会话里没人能确认（例如 `miyu ask`） | 当场拒绝，`by` 是内核，写给她的是 `The call was not run: it needs the user's approval, which no one can give here.`，说法 `core/tool-results/unattended` |
| 要问人，只读时访问类别要写入 | 当场拒绝，`by` 是内核，写给她的是只读那一句 |
| 要问人，别的 | 记一条 `tool.approval_requested`，`by` 是模块 `permissions`，照写 `access`、`rule`、`detail`，这次调用停下来等人 |

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
| `crates/miyu-session/src/guard/tests.rs` | 判法表的每一格（边界以外的读哪一级都放行）；实际生效的那一级，不认识的按只读；不报路径的：执行命令沙盒能用时工作区、只读都放行，用不了时都问人、不提规则、说明是 `{"sandbox":false,"tool":"shell"}`（施工 D-4 起），读写放行，联网除了完全放开都问人（施工 5-4 上）；说明并进工具交的几格、工具名盖不过、执行类才写 `sandbox`，沙盒用不了时也带标题和命令（施工 D-4） |
| `crates/miyu-session/tests/outside_sandbox.rs`（施工 D-4、5-15） | 要在沙盒外跑的：工作区问人、提的规则是放行工作目录的写、说明逐字节比；选了此目录不再询问的，以后的命令照旧在沙盒里、放行的目录能写，越过沙盒的照样问；在家目录里放行了的，文件工具写家目录别处不问、写私密目录照问（私密目录只认放在私密目录里的规则）；允许一次的那一次不带沙盒的规格、没报的照旧带、不问；拒绝的没跑、是拒绝；没人能确认的拒绝；只读不问、拒绝；完全放开不问、不带规格；碰数据根的照拒、不问；沙盒用不了时每条命令都问、说明带标题和命令；Unix 上真经助手跑 `shell`：没写的写不进工作区以外，写了的允许以后写得进 |
| `crates/miyu-session/tests/hazard.rs`（施工 5-17） | 真的 `shell`：工作区、完全放开两级删家目录都拒、不问，结果是拒绝、带那一句；`git push --force` 两级都问、不提规则、说明带 `hazard`、`command`；只读不看；平常的命令完全放开照旧不问 |
| `crates/miyu-session/src/guard/tests.rs` 的 `a_hazard_never_softens_a_refusal`（施工 5-17） | 报的路径已经拒了的照拒；灭顶的拒、拿工具交的那一句；别的问人、不提规则、说明带种类 |
| `crates/miyu-policy/src/hazard/tests.rs`、`hazard/lex/tests.rs`（施工 5-17） | 三类各几条认得出、拒的和问的分对；连写、外壳、`sh -c`、`eval`、替换里的认得出；平常的删、git 不误报；拆命令的边界、引号、反斜杠、注释、替换、重定向（读进来的、文件描述符不是参数） |
| `crates/miyu-session/tests/guard.rs` | 相对路径照工作目录接、工作区里还不存在的也能写；越界的读放行（施工 5-4 上）；越界的写问人，规则和说明写对了、`by` 是 `permissions`；没人能确认的拒绝；数据根哪一级都拒、写对了那一句和说法；完全放开越界的写不问；执行命令照沙盒能不能用：能用时工作区、只读都放行，用不了时问人（没人能确认就拒）；工作区里的 git 钩子只能读；几条路径照最严的，有一条在数据根里就拒、不问；指向不存在处的链接拒绝、写对了那一句和说法；写 git 钩子问人，说明里是 `read_only`、规则只有 `write`；`~` 照家目录换，写到边界以外要问人（没人能确认，是内核拒的，不是权限策略）；`trash` 判的是链接本身：工作区里指向不存在处的、指进数据根的、指到外面的链接都放行 |
| `crates/miyu-session/src/guard/tests.rs` 的 `a_venue_action_runs_only_in_a_venue`、`crates/miyu-session/tests/guard_venue.rs`（施工 O-31 前） | 在场所里做的事：场所会话（群、私聊）里放行、不问人，只读开着、没人能确认也一样；本机的会话拒绝，`by` 是 `permissions`，写给她出厂的那一句、说法对，工具没跑；载入以后照样；`venues` 有 `local` 的在本机的会话里照 `outbound`：完全放开跑，工作区没人能确认的拒、说的不是「不在场所里」（施工 O-31 补） |
| `crates/miyu-endpoint/tests/provide_venue.rs`（施工 O-31 前，O-31 补改） | 契约：扩展登记 `venue` 的工具收；`venues` 有 `local` 的本机的会话调到它照 `outbound` 问人，人点头以前扩展收不到 `tool.call`。扩展登记的工具 `venues` 没有 `local` 的本来就不在本机会话的工具面上，「不在场所里」那一句只剩给没写 `venues` 的工具兜底（`guard_venue.rs`） |
| `crates/miyu-session/tests/guard_grants.rs`（施工 D-1） | 选了本会话都允许，同一个目录下面的写不再问、名字只是开头一样的旁边的目录照旧问；家目录里的一个文件只放它本身、同在家目录里的别的照旧问；家目录在链接后面也照真实的位置比（只在 Unix 上） |
| `crates/miyu-session/tests/guard_checked.rs`（施工 5-18） | 判过的真实位置交到工具手里：报的几条照先后（相对的、外面的、还不存在的），不报路径的是空的；脚本里调的一样带；工作区里的链接在问人时被换成指进数据根，人点了允许，`write` 不写、说路径在判过以后变了（只在 Unix 上） |
| `crates/miyu-session/tests/guard_dirs.rs` | 加进来的目录（施工 5-10 上）：里面写不用问、里面的 `.git/hooks` 只能读、只读照旧拒绝 |
| `crates/miyu-session/tests/sandbox_workspace.rs`（施工 5-14） | 工作目录是家目录的：沙盒能写的是会话的默认工作区（用到时建好）、命令照旧从家目录起；子会话、孙会话写主会话的那一个，`workspace_from` 记下；载入以后文件工具写会话的那一个放行、家目录别处要问；Unix 上有收紧手段的真跑：`pwd` 是家目录，家目录写不了，会话的那一个写得了 |
| `crates/miyu-session/tests/guard_private.rs`（施工 5-13） | 私密目录：工作区、只读两级的读都问人，说明里是 `private`、规则放行整个目录；没人能确认的不跑；只读的写不问直接拒；完全放开读写都不问；工作区、加进来的目录在里面的照工作区；受保护的路径照这一轮的配置：人加的写问人、说明里是 `read_only`，读照常，去掉的出厂项写不问（施工 5-17 下） |
| `crates/miyu-fs/tests/boundary.rs` | 工作区能读能写、git 钩子和配置只能读；数据根在临时目录里也不能碰；工作区退回到数据根里照样是工作区；临时目录能读能写、系统和工具链只能读、别的在边界以外；不存在的一片不算；大小写不分的平台上数据根不分大小写；这一片自己也算；这台机器的系统目录、临时目录 |
| `crates/miyu-fs/tests/resolve.rs` | 相对路径、`~`、还不存在的照上级、链接照指向的地方、Windows 的两种分隔符和目录联接 |

### 出处

- `11-权限与沙盒.md` 第二节：三个权限级别、只读是叠在上面的开关、沙盒能用时执行命令不问、用不了时问人、「第一版的权限策略怎么判」（判法表、换不成就拒、放行规则、给头看的说明）；A2、A12、A13。
- `11-权限与沙盒.md` 第四节：边界、几片重叠时谁说了算、第一版的清单、`.git` 的钩子和配置只读、当前目录太宽；A4。
- `11-权限与沙盒.md` 第七节、A9：核心进程里的文件工具由内核照同一份边界查路径；A11：没有确认界面的场所，要确认的一律拒绝。
- `05-内核接口.md` 第五节第 1 条：工具执行前的链，最严者胜；第六节：要碰的路径。
- `02-内核.md` 第六节「确认怎么走」：放行、拒绝、要问人以后内核怎么走。

### 还没有的

- 照命令开头的放行规则（`11-权限与沙盒.md` 第二节）。越过沙盒问人放行一次施工 D-4 做了（第四条）。
- 工具链目录的缓存可以写（`11-权限与沙盒.md` 第四节的表）：现在这几片对文件工具只能读，写要问人。
- 成员和外部身份：成员只能碰自己的工作区、只在管理员的上限内批准（`11-权限与沙盒.md` 第三节）。
- 别人分享来的工作区加进边界（`11-权限与沙盒.md` 第四节）。
- 链上别的守卫，守卫超时按拒绝算（`05-内核接口.md` 第五节第 3 条）。
- 边界是策略数据、配置那一步能改（`11-权限与沙盒.md` 第四节）：现在清单写在代码里。
