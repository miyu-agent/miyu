## 施工单 AU-P0：外部审计高危十四条核实

状态：做下来了，待验收。项目主人 2026-11-18 拿到一份外部审计报告（gemini 写的，不在仓库里，不一定属实），要求二次核实、不能只听一家：第一章核心仓库 P0 的 #001–#014（加上同在画图库里的 #019）逐条在 main 上复现或照代码核对，结论分成立、部分成立、不成立；成立的照「改前红」修，不成立的不改代码。编号照审计号写成 AU-001 这样。#007 在危险命令的词法分析里，和 RO-4 同在 `miyu-policy` 的 hazard：只改 `lex.rs`、`lex/tests.rs`、`tests/audit.rs`，RO-4 没碰这三个文件，放在最后一个提交。

项目主人 2026-11-18 定：#006 远处的 MCP 服务把核心引到私网地址不挡（内网服务、授权服务器放私网是正常用法）；#008 本地文件夹装包时指到数据根外面的链接照旧拷进来，只拦走进数据根的（自己选的文件夹，和装第三方扩展自愿担风险一致）。

### 结论（一条一行）

| 编号 | 报告说的 | 结论 | 做了什么 |
|---|---|---|---|
| AU-001 | 关起来的会话把工作区的 `.tmp` 换成链接，下一条命令就能读写数据根 | 成立 | `.tmp` 不单独进名单，链接、文件换回真的目录 |
| AU-002 | 关起来的会话的工作目录宽（家目录、根目录）时整个放开 | 部分成立：外部的人改不了工作目录，只有人、场所的头换过去才会 | 宽的工作目录退回默认工作区，宽的加进来的目录不给 |
| AU-003 | 307、308 跳转把 key 带去别的主机、明着发 | 成立（三处都对） | 跨过一次主机后面每一跳都去掉；认证头、另配的头都去掉；换协议算跨了主机 |
| AU-004 | `block-beta` 的 `columns` 写多大分配多大，abort 带走核心 | 成立 | 列数最多 1024 |
| AU-005 | 时序图 `box` 的颜色绕过 MS-1，往 SVG 里写元素 | 成立 | 颜色照样过样式值过滤 |
| AU-006 | MCP 登录时远处的服务能让核心连本机端口，没有时限、没有大小上限 | 成立（私网地址一半不改，见下） | 本机地址只在服务自己在本机时认；30 秒时限、1 MiB 上限 |
| AU-007 | 词法分析在 `$'…'`、`"$(… \" …)"` 处失步，绕过 `rm -rf /` 拦截 | 成立 | 认 `$'…'`，`$( )`、反引号里认反斜杠 |
| AU-008 | 装包拷链接、`SKILL.md` 是链接、本地扫描绕回自己栈溢出 | 部分成立：前两条成立，第三条不成立 | 链接走到数据根里的整个不装；`SKILL.md` 照篱笆查 |
| AU-009 | `~//etc/passwd` 丢了家目录落到根目录 | 部分成立：解析错了，但不是越权 | 连着的分隔符都去掉 |
| AU-010 | CLIXML 的 `_x12中` 切在字中间 panic | 成立 | 先认四位十六进制再切 |
| AU-011 | 收尾标签拆在两块字里绕过中和 | 成立 | 几块字接起来再一起中和 |
| AU-012 | Windows 上 `carve` 大小写敏感，切碎数据根 | 不成立 | 不改 |
| AU-013 | Responses 接口长工具参数被当成空闲超时掐掉 | 成立 | 攒着的参数也算有动静 |
| AU-014 | Windows 打开浏览器用 `cmd /C start`，`&` 断开、能注入 | 部分成立：网址都是本机拼的，断开成立，注入要看头拿它开什么 | 改用 `rundll32`，同 `miyu mcp login` |
| AU-019 | 画图库几处反着切范围 panic | 成立（已被 `catch_unwind` 接住，影响是画不出） | 照空的标签画 |

### 逐条

**AU-001**：`confined_call` 先在核心这边 `create_dir_all(<工作目录>/.tmp)`，再把 `.tmp` 原样放进只准读、能写的名单。命令能写工作区，删掉 `.tmp` 换成指向数据根的链接（相对路径就行，场所的工作区在数据根的 `workspace/` 里），下一条命令时 `create_dir_all` 见到指向已有目录的链接直接成，助手照路径打开（Linux `PathFd::new` 不带 `O_NOFOLLOW`）顺着链接，链接指到的目录整个能读能写；写了 `read` 的规格不另挖 `hidden`。真跑沙盒助手照报告的脚本复现（沙箱目录里，链接只指沙箱里的假数据根）：第二条命令读出了假密钥、写进了文件。修：`.tmp` 在工作目录里，工作目录已经在名单里，不再单独进名单；`.tmp` 是链接、文件的先删掉再建成真的目录，不顺着链接在核心这边建。修以后同一个脚本第二条命令 `Permission denied`。

**AU-002**：`Sandbox.workplace` 关起来的会话确实没读。造会话时场所会话宽的工作目录已经退回默认工作区（`sessions/places.rs` 的 `opening_cwd`），外部的人改不了工作目录；但人、场所的头之后用 `session.set_workspace`、`session.send` 的 `cwd` 换过去的（宽的标 `wide` 照用），沙盒原样放家目录，数据根跟着露出来。修：规格里工作目录、加进来的目录换成真实位置以后照 `miyu_fs::broad` 查，工作目录宽的退回默认工作区（`HOME` 跟着），加进来的目录宽的去掉；没有默认工作区可退的不跑。不照只在 Windows 上算家目录的 `too_wide`：权限策略那边照旧，文件工具另有数据根的边界表管着，沙盒这边名单里一条包住数据根就藏不住。

**AU-003**：三处都在 `resend.rs` 读得到：去掉的只有 reqwest 那五样，`x-api-key` 照带；每一跳从原来的头起、只和上一跳比，A → B → B 的第二跳又装回 `Authorization`；只比主机、端口，同一个端口从 https 换 http 不算。测试里两个本机假服务器复现了前两条（第三条写成单元测试）。修：造请求时驱动给的认证头里值带着 key 的（`anthropic-version` 这类照带）、端点另配的头都标敏感，跨了主机连同标敏感的一起去掉；跨过一次以后后面每一跳都算；协议换了也算跨了主机。前提是第一跳那家本来就拿到了 key，所以泄露给的是它跳去的另一家。

**AU-004**：`columns 99999999999` 进 `layout/block.rs` 的 `vec![0.0f32; columns]`，400 GB 的分配失败走 `handle_alloc_error` 直接 abort，`catch_unwind` 接不住。测试里修以前整个测试程序被 abort。修：解析时列数最多 1024（`MAX_BLOCK_COLUMNS`），跨几列的照旧被列数夹住。

**AU-005**：`parse_sequence_box_line` 读出的颜色没过 `safe_style_value`，渲染时原样拼进 `fill="…"`。复现出了报告那一段。修：颜色照样过 `safe_style_value`，带引号、尖括号、`&` 的当没写颜色。`%%{init}%%` 里的主题变量同样会进属性，但 `render_with_options` 不用它（只解析不套用），走不到。

**AU-006**：`discover` 照挑战里的 `resource_metadata` 直接取，`authorization` 对端点一律放行本机地址（注释写着「给测试」）。测试里别的机器上的服务（`https://mcp.remote.example`）在挑战里写本机端口，核心照着去连了，而且不回就一直等。修：`reachable_from(服务, 地址)`：别处的只认 https；本机的只在服务自己就在本机时认（本机起的服务、测试照旧）；挑战里的、`authorization_servers` 写的、元数据里的端点都照它查（端点照授权服务器查）。每次往来 30 秒、回答 1 MiB。报告说的私网地址（`192.168.x`）没挡：公司内网的服务、授权服务器在私网上是正常用法，挡了就连不上；这一半交项目主人。

**AU-007**：读 `lex.rs` 确认两处：`$` 后面跟 `'` 时先记一个 `$`、再照普通单引号读（不认 `\'`）；`until_close` 在引号里只认同一个引号收尾，不认反斜杠。报告的两条命令修以前 `assess` 都交回 `None`。修：`$'…'` 单独读，反斜杠转义下一个字，带转义的词同变量一样记成说不清（`$'\x2f'` 就是 `/`，删它照说不清问人）；`$( )` 里不在引号、双引号、`$'…'` 里的反斜杠转义下一个字；反引号里 `\`` 不收尾。原有 hazard 测试一条不变。

**AU-008**：
1. 成立：`copy_dir` 跟着链接拷（施工 F-8 有意的：自己写包时把程序链接进来）。下载的包文件解包时只认目录和普通文件，带不进链接；照文件夹装的（`miyu pkg install <文件夹>`）带得进来，链接指到数据根里的密钥文件，装完就是 `opt/<编号>/` 里的普通文件，开着的技能文件夹关起来的会话也读得到。修：每一项照真实位置查，落在数据根里、又不在包文件夹自己里面的整个不装（报 `internal_error`，日志写是哪一项）；指到家目录这类包住数据根的，走下去照样拦住。指到数据根外面的照旧拷（主人自己的文件夹，主人担）。
2. 成立但影响小：`skills::find` 只查文件夹那一层，里面的 `SKILL.md` 是指到数据根别处的链接照读；要读出东西，指到的文件还得是写对了的技能开头。修：`SKILL.md` 的真实位置照同一道篱笆查。
3. 不成立：`ln -s . loop` 让路径一级级长出 `loop/loop/…`，每一级都是链接，内核解析一条路径最多跟 40 个链接（macOS 32 个），到了报 `ELOOP`，`walk` 当场交回错，不会栈溢出。临时探针（没提交）实测：见下。

**AU-009**：`tilde` 只去一个 `/`，`~//etc/passwd` 剩下 `/etc/passwd`，`Path::join` 丢了家目录。解析结果确实错（shell 里是家目录下的 `etc/passwd`），但不是越权：她本来就能直接写 `/etc/passwd`，权限策略、沙盒都照解析出来的真实位置判。修：连着的分隔符都去掉。

**AU-010**：`get(..5)` 成了、`split_at(4)` 切在字中间 panic，命令自己印一段 `#< CLIXML` 就能触发，哪个系统都走这一段。修：先取四个字节、要都是十六进制（ASCII，一定在字边界上），再看第五个字节是 `_`。

**AU-011**：每块字各自 `defuse` 再接起来；附件块挪到后面，隔着一张图的两块字就挨在一起。修：几块字先接起来，整段一起中和。不拆开的输出一个字节都不变（前缀照旧）。

**AU-012**：不成立。`carve` 只在 Windows 的助手里用，只处理规格里的 `hidden`；会写 `hidden` 的只有关起来的会话（`confined.rs`），而关起来的会话在 Windows 上没有命令（`CONFINES_READS` 是假，权限策略先拒），本人只读什么都不藏（施工 RO-2）。全仓库搜过构造 `Spec` 的地方，Windows 上 `hidden` 永远是空的，这段代码走不到。以后 Windows 上要藏东西时再一起处理大小写（`internal/blueprint/sandbox/windows-run.md` 已写明这一套照旧留着）。

**AU-013**：`send.rs` 只在解出增量时重新算空闲，Responses 驱动把工具参数攒到这一项完了才交（施工时定的：有的中转站流过来的增量丢了开头），攒的那一阵没有增量。测试里参数一片片来、每片间隔 60 毫秒、空闲 200 毫秒，修以前 `stalled`。修：`Decode::stirred`（默认假），Responses 攒进参数就记下，执行器读到就重新算空闲；保活注释照旧不算。

**AU-014**：`SystemBrowser` 是给头的门面（网页、im 用它开带一次性码的本机网址）：网址都是本机拼的，`&` 断开成立（多参数的网址），注入要看头拿它开什么，现在没有开别人给的网址的。修：照 `miyu mcp login` 的写法改用 `rundll32 url.dll,FileProtocolHandler <网址>`，网址整个是一个参数，不经过任何 shell。只有 Windows 的 CI 验得到。

**AU-019**：`[/]` 同时合「`[/` 开头、`/]` 结尾」，切出 `2..1`；`service svc][foo` 的 `]` 在 `[` 前面；图标的 `)` 在 `(` 前面。三种修以前都 panic，被 `miyu-mermaid` 的 `catch_unwind` 接住当画不出。修：这几处切之前先看起止，反着的当空的。

### 蓝图改了哪几节

- `session/tools.md` 第 1a 节第 1、3、7 条：`.tmp` 不进名单、换回真的目录；宽的工作目录退回默认工作区，宽的加进来的目录不给。
- `http.md`「怎么走」第 3 条重发、第 6 条空闲，「守着它的」两行。
- `mcp.md`「怎么走」第八条第 2 条、待定那一节「授权服务器只认 https」。
- `mermaid.md` 文件表、测试表；`third_party/README.md` 第 8 到 10 条。
- `fs.md` `tilde`；`packages.md`「装卸」第 1 条；`skills.md`「技能」第 2 条；`tools/shell.md` PowerShell 一行、高危命令「拆命令」；`kernel/request.md` 中和；`drivers/openai-responses.md` 事件表；`client.md` 门面表。

### 不做什么

- AU-006 的私网地址不挡（见上）。
- AU-012 不改。
- AU-008 第 1 条指到数据根外面的链接照旧拷。

### 验收

1. 每条成立的都有修以前红的测试（本机先在没修的代码上跑过，红的原因对得上）：`miyu-session` 的 `sandbox::confined::tests`、`miyu-http` 的 `resend` 两条和 `responses_idle`、`src/resend/tests.rs`、`miyu-mermaid` 三条、`miyu-mcp` 的 `oauth_reach` 三条和 `a_remote_authorization_server_cannot_name_endpoints_on_this_machine`、`miyu-store` 的 `packages::tests::links` 两条和 `a_skill_file_linked_into_the_data_root_is_skipped`、`miyu-fs` 的 `several_separators_after_the_tilde_still_mean_home`、`miyu-basesystem` 的 `an_escape_cut_inside_a_wide_character_is_left_alone`、`miyu-assemble` 的 `a_close_split_across_two_text_blocks_is_defused_too`、`miyu-policy` 的 `escaped_quotes_do_not_lose_track_of_the_line`、`audit_quotes_that_used_to_swallow_the_rest_of_the_line`；AU-014 的测试只在 Windows 上红得出来。
2. 真跑：沙盒助手照报告的两步脚本（沙箱目录里），修以前读出假密钥、修以后被拒。
3. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。
