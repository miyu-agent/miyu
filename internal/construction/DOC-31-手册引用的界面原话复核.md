## 施工单 DOC-31：手册引用的界面原话复核、网页界面一页对代码、内部文档清理后的遗留

状态：做完了一部分，等合并（分支 `step/DOC-31-手册引用的界面原话复核`，从 main 的 ebe63021d 开）。主会话 2026-11-22 派：发版前把手册里没有清完的收尾。同一天项目主人收回了并行的额度，终端界面、通讯平台、托盘三页的核对中途停下，已经查出的列在「没有做完的」一节。

### 一、手册引用的界面原话

用各组件仓库最新主分支的字表和源码重新导出底库，`uiscan.py` 扫出手册里没有逐字找到原文的引用：中文 124 条、英文 120 条。两个子代理逐条核对（每条给出模板的出处，或者程序现在的原话），真正过时的 11 条，其余是把占位符填成例子的引用、几段拼成的句子和手册自己的示例。我回各仓库的字表核对了这 11 条再修改，中英一共 17 页。

| 页 | 原来 | 现在 | 出处 |
|---|---|---|---|
| 终端界面、网页界面、权限三页（中英） | `⏵⏵ 开放权限`、`⏵⏵ Full access` | `▣ 开放权限`、`▣ Full access` | 终端界面默认主题和网页界面的 `layout.json` 换了图标 |
| 网页界面（中英） | Windows 上交给浏览器的命令是 `cmd /C start "" <网址>` | `rundll32.exe url.dll,FileProtocolHandler <网址>` | 核心 `crates/miyu-client/src/open/browser.rs` |
| 迁移、排查（中文） | 迁移程序的七句英文报错 | 换成它在中文环境下说的中文句子（`旧版 Miyu 还在运行，请先停止它` 等） | `miyu-migrate` 的 `words/cli.rs`：迁移程序按系统语言说话 |
| 迁移（英文） | ``run this as `miyu migrate <old folder>`: MIYU_STATE_DIR is not set`` | ``MIYU_STATE_DIR is not set; run this as `miyu migrate <old-folder>` `` | 同上 |
| 自动化（中英） | 终端界面输入框上面那一行写着 `长期目标 · 第 N 轮` | `◎ 目标 · 进行中 · 第 N 轮`，后面是用时；状态变了换成 `已暂停`、`等待回复`、`受阻` | `miyu-tui` 的 `src/ui/goal_tag.rs` |
| 通讯平台（中文） | 卡片上显示 `在 <地址>:<端口> 等待 NapCat 连接` | `等待 NapCat 连接` 和 `端口 <端口>` | `miyu-im` 的 `im/site/pages/home/home.js` |
| 工具、附件参考（中英） | `文件已变`、`文件已不在`、`The file is gone` | `文件已更改`、`文件已不存在`、`The file no longer exists` | 核心的拒绝原因（WD-5）和两个界面的字表 |
| 清单字段、软件包（英文） | 照中文自己译的两句 | 换成程序的英文原话 | `crates/miyu-cli/src/language/confirm.rs`、`pkg.rs` |

### 二、网页界面一页对代码

`docs/zh/guide/heads/web.md` 整页和工具页的「ask_user」一节，由子代理读 `miyu-web` 最新主分支的代码逐条核对：摘出 272 条说法，250 条和代码一致，16 条不对，1 条在代码里找不到，5 条读代码判断不了。我抽读了其中六处代码（空会话的 `选项` 按钮、组件开关存在核心里、链接过期提示的场合、删除项目的确认、外观包的提示、确认第一行的写法），和子代理的结论一致，然后中英两页一起修改：

- 窄屏的边界是「不超过 836 像素」；悬停才出现的按钮一直显示的条件是触屏设备，不是窄屏。
- 引导：`跟随系统` 是标在和浏览器语言相同那一行上的记号，不是单独的选项；「其他设备」一步并不只在 `miyu web` 打开时才有；先设置访问密码的情形是用 `miyu web --reset` 的一次性码链接；更换界面语言后从下一步继续。
- 子代理会话里输入框上方的按钮写的是 `返回`；空会话上方是一个 `选项` 按钮，菜单里有 `人格`、`预设`、`工作区` 三行。
- 右侧边栏：可以搜索的是「展示文件」卡；点文件修改卡里的文件直接进放大的一页。
- `N 个后台任务` 和待办在窗口放得下右侧边栏时不画在输入框旁边，在右侧边栏的卡片里。
- 确认的第一行：没有 `写入 N 个文件` 这种写法，实际有四种（命令的短标题、`安装软件包 <编号>`、`<软件包> 请你确认：<工具>`、`使用 <工具名>`）。
- 项目：留言开关叫 `跨项目留言`；删除项目是弹确认框，不是点两下。
- 网页组件的开关保存在核心里，各设备看到同一份；安装、卸载外观包当场生效，升级或重装已加载的才提示刷新；`这个链接已用过或已过期` 是一次性码链接（5 分钟）失效时的提示，不是带票据的链接。

「ask_user」一节的 9 条都和代码一致。

### 三、内部文档清理后的遗留

SCRUB-1 到 SCRUB-4 之后，子代理按模式全量搜了 `internal/`（顺带看了 `docs/`、`crates/` 的注释），并把 SCRUB-3、SCRUB-4 改过的每一处在现在的文件里读了一遍。查出漏掉的 30 处、改坏的 22 处，这一步全部修改；拿不准的 16 处没有改，列在下面。

漏掉的：

- **局域网测试机的称呼**，10 处：SCRUB-4 之后的 WD-5、WD-6、LW-1、LW-2、LW-3 又写了进来（WD-5、WD-6 是这条文档线自己写的）。照 SCRUB-3 改成「另一台测试机」。门禁「私密路径」只认路径的形状，拦不住这种写法。
- **真实的 QQ 号码**，3 处（设计 68、施工单 E-4 上、E-1 修二）：换成占位。
- **渠道前缀 `group/`**，14 处：SCRUB-3 只改了写着中转站名字的句子，写成「开发端点 …」的没有改。登记簿 `internal/designs/26-提示词.md` 改了以后，`internal/blueprint/prompts.md` 用 `cargo xtask prompts` 重新生成；`crates/miyu-memory/tests/budget.rs` 的一处是注释。
- **其余**：一处小写的渠道名、两处不公开报告的内部代号、设计 43 里留着的一个作者名（改成「一个开源的本地网关」）。

改坏的：8 处是替换以后读不通或者意思变了（例如「`中转站` 是中转站」「中转（中转站）」「免费的 `免费模型`」、引用的运行日志里 `provider=` 后面成了中文），按原意改写；14 处只是替换以后多出一个空格。

### 没有做完的

1. **终端界面、通讯平台、托盘三页对代码**：两个子代理做到一半被停下。终端界面一页核对到第 283 行（154 条，13 条不对），通讯平台和托盘两页核对到通讯平台的第 199 行（121 条，8 条不对），托盘一页还没有开始。已经查出的 21 条不对的**这一步没有修改**，列在下面；每条的依据和建议的改法在 `~/.cache/miyu-docs-2026-11-14/doc31/behavior-tui.json`、`behavior-im-applet.json` 里。这些结论只是读代码得出的，改之前需要像网页那一页一样抽读代码确认。

| 页和行 | 手册的说法 | 代码实际的行为（依据） |
|---|---|---|
| `heads/tui.md`:18 | --resume <编号>：打开这个会话，编号要写完整；如果打不开，就报错退出 | miyu-tui/src/startup.rs:98 编号过不了 SessionId::parse（不完整）时才印用法、退出码 2（:63-73）。编号写法对、但会话不存在时不退出：src/core/spawn.rs:112 注释「不验证」，核心答 session_not_found 后 src/core/link.rs:70-78 发 SessionGone，src/app/session.rs:95-105 在正文末尾红字写「会话不存在」（zh.jso |
| `heads/tui.md`:82 | 点击「测试连接」 | miyu-tui/src/app/oobe.rs:96-101 引导里鼠标事件只记指针位置（Moved、Drag），点击不处理；「测试连接」要把光标移到那一行按 Enter（src/oobe/model/mod.rs:258 Slot::Test 在 Enter 分支里；提示行 zh.json:1537「Enter 测试连接 · j/k 移动 · Esc 更换供应商」） |
| `heads/tui.md`:84 | Windows 上没有外部编辑器这个选项，需要在编辑窗口里编写或者粘贴进去 | miyu-tui/src/oobe/sheet.rs:61 Ctrl+G 没有平台分支；src/app/oobe.rs:120-131 用 editor::or_default()，src/editor.rs:18-20 Windows 上没设 $VISUAL、$EDITOR 时用 notepad，:43-49 经 `cmd /S /C` 起；提示行 zh.json:1545「Enter 换行 · Ctrl+G 外部编辑器 · Esc 完成」也不分平台（提 |
| `heads/tui.md`:85 | 如果人格没有头像，会多一项「形象」，可以选择显示或不显示吉祥物（开启会提高 CPU 占用，拖慢启动速度） | miyu-tui/src/oobe/theme.rs:210-226 slots() 不按有没有头像筛「形象」那一格；src/ui/oobe/theme_step.rs:81-86 这一格一律画（注释：2026-10-29 改名「形象」，头像也归它管）；选「不显示」时有头像的也不画头像（src/oobe/mod.rs:413-418）。说明那一句 zh.json:1693 逐字一致 |
| `heads/tui.md`:85 | 选择跟随头像、跟随壁纸时，会多一项「主色」 | miyu-tui/src/oobe/theme.rs:202-204 seeding()：配色在 SEEDED 里（src/theme/choice.rs:50 = persona、wallpaper、terminal）并且有候选主色时才有这一格，终端配色也算 |
| `heads/tui.md`:113 | AI 回答时的运行状态行显示「思考中」「回答中」和用时 | miyu-tui/src/ui/status.rs:35-76 运行状态行写的是从词库里轮换挑的一个词（resources/pulse.json：`深度求索`、`思考中`、`斟酌中`、`琢磨中`…，30 秒、90 秒以后换另一组），后面接一到三个点和用时，重试时再接重试那一句；它不按「思考/回答」分。`回答中`（zh.json:617 jobs.doing_replying）只用在子代理状态行（src/app/sessions/notes.rs:30） |
| `heads/tui.md`:115 | 底栏右边是速度和用量那一格（例如 ◔ 128.4k · ¥0.42） | miyu-tui/src/ui/footer.rs:44-74 右段只有软件包的界面块、「自动朗读」和用量那一格，没有速度（grep speed、tok/s 在 footer.rs、usage_cell.rs 里没有；速度只写在一轮结束的收尾行，src/transcript/speed.rs:53）。用量那一格 src/ui/usage_cell.rs:122-146：环、累计 token，记到过输入时紧跟缓存命中率 `(C{percent}%)`（zh. |
| `heads/tui.md`:115 | 窗口变窄时，先去掉右边的格子，再把权限级别收成只剩图标、模型只显示名字的最后一段，最后去掉供应商名 | miyu-tui/src/ui/footer.rs:213-241 split() 的先后是：① 模型编号只写最后一段，同时右边先丢界面块、花费这几格（keep≤2，EARLY）；② 权限级别收成只剩图标；③ 去掉供应商名；④ 还放不下再丢右边剩下的格子，用量的环和累计留到最后（usage_cell.rs:135 keep=4） |
| `heads/tui.md`:124 | 「侧边栏形象」关闭时，如果人格有头像，仍然会显示头像 | miyu-tui/src/ui/avatar_view.rs:27-30 slot_shown() 只看开关（注释：2026-10-29「关了形象头像也不该显示」），:67-70 wanted()：开关关着的什么都不画，头像也不画 |
| `heads/tui.md`:128 | usage：「用量」，一共多少 token、输入和输出各多少、缓存命中率、压缩了几次、缓存断裂了几次、花费 | miyu-tui/src/ui/sidebar.rs:115-140 usage_lines() 只有共多少、输入输出、缓存命中、缓存断裂，后面接花费（parts.rs:156-160）；「压缩 {n} 次」写在上下文那一块（sidebar.rs:104-110） |
| `heads/tui.md`:182 | Windows 上没有 Ctrl+G 这个功能，按下去不起作用 | miyu-tui/src/app/keys.rs:176-179 Ctrl+G 没有平台分支；src/editor.rs:18-20 Windows 上没设 $VISUAL、$EDITOR 时用 notepad，:43-49 经 `cmd /S /C` 起（提交 4314cba6，2026-11-15）；空输入框的提示 `Ctrl+G 用编辑器写这一句`（zh.json:16）在 Windows 上也不去掉（src/tips.rs:19-21 只去掉 Ct |
| `heads/tui.md`:193 | 往上翻时，右下角会显示「跳转到底部（Ctrl+End）↓」，有新内容时它会变亮 | 字逐字一致（zh.json:101），有新的字以后换强调色（miyu-tui/src/ui/bottom_button.rs:36-38）；但位置不在右下角：:14-22 place() 画在输入框上面紧挨着的那一行、照输入框居中（:1-3 注释：2026-10-19 从正文区右下改过来）。点它也能回到底部（src/app/mouse.rs:83-88） |
| `heads/tui.md`:231 | 确认框上有三个选项 | miyu-tui/src/drawer/texts.rs:40 `decisions: [String; 2]`，字表 zh.json:655-658 只有「允许本次」「不允许」两项；手册的权限那一页（docs/zh/guide/permissions.md:104）写的也是两个选项 |

| 页和行 | 手册的说法 | 代码实际的行为（依据） |
|---|---|---|
| `heads/im.md`:26 | 确认屏上的 `需要的能力` 会列出：提供工具、操作会话、代平台用户发言、读取会话、写入会话记录、联网、调其他软件、往系统提示词里放内容、在界面上显示内容、替本机页面换登录票 | miyu-im/im/package.toml:35 的十项能力和手册的先后对得上；核心 resources/core/human/zh.json:584-619 的名字里 packages.call 是「调用其他软件包」（:606），不是「调其他软件」，其余九项逐字一致；标题「需要的能力」在 crates/miyu-cli/src/language/pkg/info.rs:62 |
| `heads/im.md`:40 | `miyu pkg page im` 作用同上（同 `miyu im web`：im 没有启动时先启动它） | 核心 crates/miyu-cli/src/pkg/page.rs:47-69 后台页由程序自己提供的包只问它的 status 拿端口，程序没在运行时报错退出，不会启动它；报错是 crates/miyu-cli/src/language/pkg.rs:262「im 未运行，启用它以后再打开」。打开浏览器、带登录票这两点和 `miyu im web` 相同（page.rs:70-72、:39-41） |
| `heads/im.md`:112 | 在私聊里输入 `/` 会列出 Miyu 的命令 | miyu-im/crates/miyu-im/src/core/route/menu.rs:1-3、:40-42、:52-72 命令菜单只交给 external.bindings 里绑定了的号（你本人）；crates/miyu-im-telegram/src/link.rs:195-197 setMyCommands 的 scope 是这几个号各自的私聊。没绑定的人的私聊里不会列出 |
| `heads/im.md`:121 | 后台页的 `设置 → 绑定 → 本人的平台 ID` | miyu-im/im/site/pages/settings/settings.js:34-39、:79 设置页左边一列的四段是 bindings、general、appearance、site，名字照 settings-page/<编号>：texts.js:2801「本人的平台 ID」、2804「通用」、2796「外观」、2817「访问」。没有叫「绑定」的一段，「绑定」是按钮上的字（texts.js:2797） |
| `heads/im.md`:121 | 点击 `新绑定`，先选择平台，再填写号码 | miyu-im/im/site/pages/settings/settings.js:134-137 这一行的名字是「新绑定」，能点的按钮上写的是「绑定」（texts.js:2800、2797）；pages/settings/bind.js:12-33 第一步选平台、:34-66 第二步填号，最后的按钮也是「绑定」 |
| `heads/im.md`:162 | 私聊出厂：每条有文字的消息都回复；只有图片、表情的消息只记录 | miyu-im/crates/miyu-im/src/core/route/private.rs:35-52 私聊只有既没有字、也没带任何东西（图、表情、文件、引用、拍一拍）的才不送，只带图或表情的照样交给 AI 开一轮（:79「私聊每一条都开回合」）。「只有图、表情的只记下」是群里线路规程 every-message 的做法（core/route/discipline.rs:94、called.rs:342-347），私聊这条路不看线路规程 |
| `heads/im.md`:175 | 概况页有 `是否回复` 开关（关掉以后只记录不回复） | miyu-im/im/site/pages/venues/venue.js:226-240 群卡、陌生群、陌生私聊的概况有这个开关（texts.js:3185-3186「是否回复」「关闭后只记录消息，AI 不回复」）；私聊白名单里的卡没有（one.on 是 null），那里写的是「在私聊白名单里」和一句说明 |
| `heads/im.md`:182 | 已开启的插件在这个会话里添加的页，例如 `群管理面板` | miyu-im/im/site/pages/venues/venue.js:176-185 卡片左栏「来自插件」下面的页签写的是插件的名字（pick(one.name)），群管理插件的名字是「群管理」（crates/miyu-im-onebot/src/admin.rs:93）。「群管理面板」是这个插件另一个整页的标题（admin.rs:113-117），出现在侧栏的「软件后台」里（im/site/pages/backstage/backstage.js: |

2. **网页界面一页读代码判断不了的 5 条**（靠样式推断的、要真机看的）和「代码里有、手册没写」的 12 条，在 `behavior-web.json` 里，没有处理。
3. **内部文档里拿不准的 16 处**，需要主会话决定：
   - `internal/` 里保留着的会话名（SCRUB-3 明写不改，七类，各举了一处）。
   - 把一个具体的开源网关当先例引用的四句，换成泛称以后指代变虚（`Q-4下`、蓝图 `models.md`、设计 `README.md`、`once_probe.rs` 的注释）。
   - `RP-1` 第 10 行：去掉渠道前缀以后，「这个名字目录里认不出」这句话可能和事实不符，没有运行验证。
   - 设计 10 第 260 行：同一次测量，代码注释和文档用了两个不同的中性名字。
   - `8-26` 第 7 行留着一个端口号；`FH` 第 195 行一处小写的渠道名可能只是公开服务名的随手写法；测试里用项目主人的网名当登录名。
4. **已经进主分支的提交说明**里也有测试机的称呼（WD-4 到 WD-7 的「门禁十二项通过」那一行），提交历史这一步改不了。
5. **各界面自己的字**里查到一处旧的：网页界面权限按钮的悬停提示还写着三级「工作区 → 开放权限 → 只读」，实际只有两级。
6. 需要 Windows、macOS 真机或者界面实际操作才能确认的条目（报告 11-15、11-16 两节列的）没有变化。

### 验收

- `python3 ~/.cache/miyu-docs-2026-11-14/lang/check.py .`：中英各 62 页，1566 个链接，问题 0 个。
- 改完以后在工作树里搜测试机的称呼和那个渠道前缀：0 处（变基时主分支新进的 LW-3 施工单里又有一处测试机的称呼，一并改了）。
- 门禁十二项、三个平台的 CI 通过（这一步攒在一起只跑一次）。
