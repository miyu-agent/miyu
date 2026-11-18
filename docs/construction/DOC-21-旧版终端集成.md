## 施工单 DOC-21：迁移页、终端界面页跟上旧版终端集成的处理

状态：待验收（2026-11-19）。迁移包发 0.1.4：报告的「数据根外面」一节多报旧版终端集成留在 shell 启动文件里的那一段（写第几行到第几行），fish 的那一份照 `XDG_CONFIG_HOME` 找、是悬空链接的也报。终端界面的 `miyu shellhook install` 现在遇到旧版的那一份会先备份再删。手册还是原来的说法（迁移只查六个位置；一句话发两遍时让人自己删 `conf.d/miyu.fish`）。

### 改了什么

- 终端界面页（中英）「shell 集成」下加「清理旧版的终端集成」一小节：旧版留在哪些文件、安装时怎么备份和清理（启动文件备份成 `<文件名>.miyu-old-hook.bak`，fish 的普通文件改名、链接只删链接）、备份重名时往后编号、哪两种情况不修改要自己处理、`status` 和 `doctor` 只报告不删、`uninstall` 不处理、Windows 上不检查。「排查问题」里「一句话被发送了两遍」那一行改成再安装一次。
- 命令行页（中英）`miyu shellhook` 的 `install`、`status` 两行各补一句。
- 迁移页（中英）：检查的位置从「六个位置」改成六个位置加 bash、zsh 的启动文件，写明 fish 那一份照 `XDG_CONFIG_HOME` 找、报告会写第几行到第几行、迁移只读不改；「迁移完成以后」加一条旧版终端集成怎么处理。
- 终端界面页英文「排查问题」里一句报错照终端现在的字表改：`The core isn't running: when starting, set MIYU_CORE_BIN to the miyu program`（原来抄的是旧字，带内部说法 `the remake's miyu`）。DOC-20 施工单「留下的」里把这一条更正了：不是程序的问题，是手册抄了旧字。

### 实测

都在 .91 上、挡住真家目录的沙箱里跑（`bwrap --tmpfs` 家目录，只把编好的程序只读放回来），沙箱家目录里放旧版的痕迹：`.bashrc` 里两段、`.zshrc` 里一段、`.profile` 里只有开头的一段、`conf.d/miyu.fish`（带旧版的戳）。

- 终端界面（main f8c9ebc6）：`shellhook status` 逐个报出旧版的那一份在哪个文件第几行；`install --shell bash` 清掉第 3–5 行、原文件备份成 `.bashrc.miyu-old-hook.bak`，别的行没变，再来一次备份叫 `.bak.1`；`install --shell fish` 把 `miyu.fish` 改名成 `miyu.fish.miyu-old-hook.bak`，换成悬空链接时只删链接；不是旧版写的 `miyu.fish`、只有开头没有结尾的一段都没动、各说了一句。中英两种语言的输出都看了，手册里引的是实际印出来的话。
- 迁移包 0.1.4（main 145cf03）装进核心（main cc886dd3a），对一个只有布局标记的旧目录真跑了中文、英文各一遍：报告「数据根外面」列出 `~/.config/fish/conf.d/miyu.fish`、`~/.bashrc`（「第 3～5 行、第 7～9 行」）、`~/.zshrc`；只有开头的 `.profile` 没报；fish 那一份换成悬空链接后英文报告写的是链接原来指到哪。迁移前后四个文件的校验和没变，没有多出备份文件。
- 没测到的：`ZDOTDIR`、`XDG_CONFIG_HOME` 指到别处时的两条，照迁移包 `leftovers/startup.rs` 和终端蓝图 `shellhook.md`「旧版那一份」写；macOS 上旧版写登录文件（`.bash_profile`）那一种照同一处写。

### 验收

1. 文档线的检查脚本问题 0 个。
2. 终端界面页有「清理旧版的终端集成」一小节，迁移页「迁移完成以后」有旧版终端集成那一条，两处互相指得到。
