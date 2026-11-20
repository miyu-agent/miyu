## 施工单 DOC-8：手册改成数据根读得到、写不进

状态：待验收（2026-11-17）。跟 RO-2：项目主人 2026-11-16 定，只读就是没有写权限的开放权限；Miyu 的数据根（连存 key 的 `etc/private/secrets.toml`）两种权限下文件工具、命令都读得到，写照旧哪一种都不行。

### 改了什么

中文页、英文页同一处一起改。

- `guide/permissions`：对照表里读那一行去掉「数据根除外」；数据根那一段重写（读得到、写不进，被拒时记「这是 Miyu 自己的数据，不能改」，密钥文件也读得到，能写的几处例外，`glob`、`grep` 从上面往下搜时不进数据根、起点在里面的照常搜）；只读下的命令「数据根也在内，沙盒不藏任何目录、只挡写」；三个平台表里 Linux 一行去掉「读不到里面」；「出了问题」表那一句照新的原文。
- `guide/security`：「挡的」表里那一行改成只挡写；「不挡的」表里写明 AI 也读得到密钥文件、读了的 key 会发给模型供应商，「你的文件发给模型供应商」一行把数据根算进去。
- `guide/knowledge`（知识库的文档读不拦、写的指去用知识库工具）、`guide/troubleshooting`（那一句的新原文）、`guide/extending`（脚本文件沙盒里照样读得到）、`guide/tools`（`glob`、`grep` 不进数据根）。

没动的：`guide/sessions`、`guide/heads/web` 里「数据根里的文件不能当附件」，头那一边照旧不给；通讯平台上别人的会话照旧只碰得到自己的工作区。

### 实测

沙箱数据根，当前 main（含 RO-2）编的 `miyu`，Linux，测试端点的模型；沙箱的密钥文件里放的是假 key：

- 只读下 `read` 读 `~/.miyu/etc/private/secrets.toml`：时间线记「读取 … · 2 行」，读到了。只读下命令 `wc -l ~/.miyu/etc/config.toml`：照常印出。
- 只读下 `write` 往 `~/.miyu/etc/x.txt` 写：「没做：只读模式，未执行」；命令 `touch ~/.miyu/etc/y.txt`：「权限不够」。两个文件都没建出来。
- 开放权限下 `write` 往 `~/.miyu/etc/x.txt` 写：「没做：这是 Miyu 自己的数据，不能改」，英文界面是 `not done: Miyu's own data, can't be changed`；没建出来。往 `~/.miyu/root/documents/ok.txt` 写：建出来了。
- `grep` 从家目录搜工作目录里才有的一个词：只搜到工作目录里的那个文件，数据根里会话日志里的那一处没搜到（同一个词用命令 `grep -r` 搜得到两处）。`grep` 的 `path` 写 `~/.miyu/etc`：搜到 `config.toml`。

没实测：通讯平台上别人的会话；Windows、macOS。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. 手册里搜不到「谁都不能碰」和「读」那一行的「数据根除外」。
3. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
