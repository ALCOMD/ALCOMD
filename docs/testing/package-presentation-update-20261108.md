# 软件包展示更新记录（2026-11-08）

所有者在完成软件包展示验收后明确要求“提交并更新”。本轮在标准 main 工作树、真实 Windows 用户环境提交并更新本机 Core/GUI，没有远端写入。

实现提交：`e0073759ab8fca713db9ca8b486d9617a526b890`（`feat(packages): complete filtered package presentation`）。构建前核对上一轮验证文件摘要全部一致，仅补充所有者本轮提交/部署授权记录。提交后重建 Core、全工作区前端和 Tauri debug no-bundle GUI，全部成功；没有修改实现来绕过验证。

| 组件 | 运行状态 | SHA256 | PID |
| --- | --- | --- | --- |
| Core | ready；State30 / Config12；候选 v2；活跃任务 0 | `72250E853B38047AFBDAA741669FBC82E398BD7828DA70318298307950B89896` | 266396 |
| GUI | 窗口已打开且响应；WebView 原 profile 匹配 | `37C506D13E14BE7B817DD1648405E0AE3E9A503BE04025B4D8EBE388AFE6A922` | 264784 |

原 Core 和 GUI 在本轮检查前均已退出，最初 RPC 探测超时后通过进程清单确认，没有强制停止正在工作的程序。原数据与 WebView profile 仍存在，继续使用 `target/m7-package-navigation-100ms-20261108/native-reopen/data` 和相邻 `webview-profile`。GUI 保持 `ALCOMD_DISABLE_VCC_REGISTRATION=1`，未操作 VCC 注册或 Unity 项目。

启动新版前复制停止状态的数据目录到 `.local/package-presentation-update-20261108/data-before`，共 8 个文件、2773563 字节，并保留两个原程序备份与 SHA256 清单。备份数据库 quick_check 为 ok，Schema29。新版正常迁移至 State30，运行时只读检查 quick_check 仍为 ok，33 张原有表按原列逐行比较的摘要全部一致，设置文件摘要也相同。没有恢复旧备份、替换用户数据或补写虚构的展示观测。

实际握手同时请求候选 v1/v2，只有 v2 被协商；system.status 为 ready，operations.list 活跃数为 0，读取到原有 1 个项目与 2 个仓库。项目候选 v2 返回 12 条 summary，新展示状态、provider 类型与顺序字段符合合同。旧项目未采集的展示信息保持 unobserved，可从 GUI 的“刷新软件包信息”调用既有 Core 用例补采。

本地验证脚本最初误用了数组能力比较，修正后只读复验通过。GUI 启动后的 WaitForInputIdle 在本宿主返回不支持，随后直接核对已有进程、非零窗口句柄、Responding 和 WebView profile 参数；没有重复启动 GUI。两组件 stderr 均为 0 字节。未执行原生窗口内部点击或新的 H7 人工验收，三平台 Hosted CI/CodeQL 未运行。

构建、备份、迁移对比、握手/候选读取、进程与窗口记录均在 `.local/package-presentation-update-20261108/`。部署源码绑定上方实现 SHA；后续文档提交只记录本次部署事实，不改变程序输入。
