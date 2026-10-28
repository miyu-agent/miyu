## 施工单 OV-1 补：覆盖放进新建的子文件夹也看得到

状态：写好了（2026-10-28）。主会话 2026-10-28 派：PT-2 上写测试时撞见，`prompts/` 下新建一个子文件夹、马上写进覆盖，核心盯着的那一头没看到，号不变，下一句不问 `prefix_changing`。先写红的测试再修。图纸 `docs/blueprint/config.md`「在哪」的 `watch.rs`，`protocol.md` 的 `prefix_ask`，OV-1 施工单第十节第 1 条。

### 为什么漏

系统的递归监视（Linux 上 inotify，`notify` 替我们往下一层层登记）报了「新文件夹」才去盯它。建好文件夹马上写文件的（编辑器、`mkdir -p` 再写、拷一整个文件夹进来），写的那一下赶在登记前头，没有事件。新文件夹那一下的事件，`Table::touched` 又照「还在的目录不算」扔了。

### 做了什么

1. `miyu-store` `watch.rs`：往下递归看的，事件是新建（`Create`）、改名挪进来（`Modify(Name)`）而路径是目录的，把它下面每一层已经有的文件都交出去（点开头的不往下看，链接到的目录不跟进去）。别的事件里的目录照旧不算（Windows 读写里面的文件时会单独报一下目录）。之后再写进去的照常报；同一份 200 毫秒里合并，多交的一下只是多重读一次。
2. 图纸 `config.md`、`protocol.md`，OV-1 施工单第十节第 1 条。

### 验收

1. `miyu-store` `watch/tests.rs` 的 `a_new_folder_in_a_watched_tree_hands_over_what_is_already_in_it`：新建、挪进来三种事件都交出里面的两份（深一层的也算），点开头的不交，还在的目录别的变动不算。修前红。
2. `miyu-endpoint` `tests/prefix_ask.rs` 的 `an_override_in_a_new_folder_is_asked_too`：`prompts/core/` 原来没有，建好就写覆盖，下一句拒 `prefix_changing`、带那份文件。修前红（十秒没看到改动），修后连跑五次都过。
3. 手写变异；`cargo xtask check`、Windows 版 clippy；三平台 CI。
