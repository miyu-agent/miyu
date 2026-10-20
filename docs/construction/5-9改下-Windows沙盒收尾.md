## 施工单 5-9 改下：Windows 沙盒收尾

状态：排着（2026-10-20），接在 `5-9改上-Windows沙盒接MXC.md` 后面。

### 目的

上半把 Windows 的命令关进 MXC 以后，收掉原来那一路留下的、改写图纸和设计，真机验收。

### 做什么

- **5-8 装的沙盒用户**（`miyu sandbox setup`、`remove`，`crates/miyu-sandbox/src/install/`）：MXC 不用它。撤不撤、怎么撤（已经装过的机器上把用户和家目录那一条可读一起撤掉）开工前列清单问项目主人（删冗余先确认），定了再动。
- **git 的 `safe.directory`**：9 月的路子是另一个用户跑，git 不认你的仓库；AppContainer 照旧是你这个用户，多半不用登记。真机上跑一遍 `git status`、`git commit`，用不着的就不做。
- **脚本调工具的通道在 Windows 上**：和 M-3 下（Windows）对好，容器里连得上那一根管道、连不上核心的主管道（设计 11 A6）。
- **设计、图纸**：设计 11 A3 记「2026-10-20 改用 MXC」；`docs/blueprint/sandbox/windows.md` 照新的写；`sandbox.md` 的平台表、「还没有的」跟着改；用户文档里 Windows 的沙盒说明。

### 验收

1. 虚拟机真机：装好的 Miyu 在桌面会话里，工作区这一级不再每条都问；她跑 PowerShell、git、cargo 照常；写不了工作区外面、读不到数据根和私密目录；SSH 连进去用时照实说没有桌面会话。
2. `cargo xtask check` 全过；CI 三台绿。
