## 沙盒：Windows 装

### 是什么

Windows 上不用装任何东西。

施工 5-8 做过一次要管理员权限的安装：`miyu sandbox setup` 建一个专用的低权限用户 `miyu-sandbox`、藏在登录界面之外，`miyu sandbox remove` 撤掉它。2026-10-20 改用 MXC 以后命令不再以它的身份跑（`sandbox/windows-run.md`）；2026-11-09 项目主人定只留只读、完全放开两级（`11-权限与沙盒.md` A16），沙盒用户连同这两条命令在施工 LV-2 删掉：`miyu-sandbox` 的 `install` 模块、`miyu sandbox` 和它的帮助页、数据根里的两份文件都没有了。

### 以前装过的机器

Miyu 不再认、也不再管下面几样，要撤的自己撤（要管理员权限）：

- 本机用户 `miyu-sandbox` 和它的用户配置文件夹：`net user miyu-sandbox /delete`，再到「系统属性 → 用户配置文件」里删它的那一份。
- 注册表 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList` 下的值 `miyu-sandbox`（登录界面上藏它用的）。
- 数据根里的 `etc/private/sandbox-windows.json`（装的记录，含 DPAPI 加密过的密码）、`var/log/sandbox/windows-error.json`（没装成时的原因）。

### 出处

- `11-权限与沙盒.md` A16：随「工作区」一级去掉 Windows 的沙盒用户和单独的桌面；A3、第六节是当初的设计。
- `sandbox/windows-run.md`：Windows 上每条命令怎么关进沙盒（MXC）。
