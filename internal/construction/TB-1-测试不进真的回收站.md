## 施工 TB-1：测试往回收站放的不进系统临时目录的回收站

状态：做完，等验收（分支 `step/CB-3`）。核心主会话派（2026-11-18），派工时叫 TR-2；TR-2 已是「软件包能往托盘菜单挂一行」，撞号改 TB-1。

### 查到的

- 另一台测试机的 `/tmp/.Trash-1000` 攒了 83 份 `dog.txt`（11-14 起），路径都是 `miyu-cli-<进程>-<时刻>-<序号>/root/kb/notes/pets/dog.txt`；本机的同一处也有，另有 `…/root/kb/mine`（同一个文件里另一条测试删的库）。
- 是 `miyu-cli` 的 `kb::building_a_base_from_the_shell`（`miyu kb remove notes pets/dog.txt`）和 `kb::mounting_a_folder_and_deleting_it_from_the_shell`（`miyu kb delete mine`）。测试的夹具 `Home` 造核心时不给家目录（`Core::new` 的 `home` 是空的），Linux 的回收站（`miyu_fs::trash`，照 freedesktop 规范）找不到家目录的回收站，就放进那块盘最上面那一层的 `.Trash-<uid>`：临时目录在 `/tmp`（tmpfs）上，就是 `/tmp/.Trash-1000`。门禁一趟留两份，测试收尾只删自己的临时目录，删不到那里。
- 产品没错：没给家目录的核心照规范放。错在测试的夹具。

### 改了什么

- `miyu-cli` 的测试夹具多 `Home::with_own_home`：家目录是这个 `Home` 自己的 `<数据根>-outside/home`，回收站落在它的 `.local/share/Trash`（同一块盘，一定放得进），收尾跟着删。
- 上面两条测试用它，另查一遍（只在 Linux：macOS、Windows 放进系统的回收站，不照交给核心的家目录）：退出码是 0、东西在自己的回收站里。原来「这块盘没有能放的回收站就照实报」那一支在 Linux 上不会再走到。

### 验收

1. 改前红：两条测试在改夹具以前跑（`TMPDIR=/tmp`），都红在「没进自己的回收站」，`/tmp/.Trash-1000/files` 从 535 份变成 537 份（多的两份是这一趟的，当场删了）；改后连跑 3 趟都过，`/tmp/.Trash-1000` 一份没多。
2. 门禁跑完以后另一台测试机的 `/tmp/.Trash-1000` 不再多出新的：见文末「实测」。
3. 另一台测试机上已有的：核对都是这两条测试的产物以后删掉，见文末「实测」。

### 不做什么

- 不改核心没给家目录时的放法（照规范）。
- 本机 `/tmp/.Trash-1000` 里以前攒的不动（不在派工里），交回时报数。

### 实测

- 另一台测试机跑这一步的门禁之前，`/tmp/.Trash-1000` 有 196 份：`dog.txt` 98 份、`mine` 98 份（`Path=` 都是 `[m<进程>/]miyu-cli-<进程>-<时刻>-<序号>/root/kb/…`，门禁的 `TMPDIR` 是 `/tmp/m<进程>`）。跑完一趟（十一项里测试那一项过了）一份没多，最新的一份还是门禁之前的。
- 已有的 `dog.txt`：98 份的 `.trashinfo` 都对得上上面那个路径、`files/` 里都在、内容都是「阿黄是小狗。」，删了 98 份和它们的 `.trashinfo`。`mine`（同一个文件里 `kb::mounting_a_folder_and_deleting_it_from_the_shell` 删的库）98 份没动，交回时报。
