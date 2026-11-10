# 完整存储库配置交换（2026-11-10）

## 结果与合同

按所有者要求，GUI 默认导出专用 `.alcomdrepositories` 文件，包含全部内置、本地及远程来源、自定义显示名、优先级、仓库开关、包开关和已有认证信息，另保留旧 VCC TXT 兼容导出。文件的包关闭清单不依赖当前索引是否存在对应包。没有把索引或软件包归档打包进配置文件。

所有者补充选择可选、默认覆盖：原生配置导入默认勾选“覆盖已有存储库配置”，按来源映射本机 ID 并恢复配置，保留未包含的其他来源；取消勾选仅添加新来源并追加顺序，已有项的名称、优先级、认证、开关及修订保持。旧 TXT 继续追加。已有来源不为恢复配置下载索引，新来源复用既有读取与登记；并发读取、串行登记，写入结果未知时停止后续登记。

Core 统一解析、匹配、认证及写入。GUI 只呈现选择和结果，并在导入结束后失效共用存储库视图，避免预加载的旧开关继续显示。名称及顺序只检查实际受影响字段，不检查无关索引修订；设置保存复用现有 Config13 CAS。数据库元数据和设置文件不是跨存储原子事务，失败明确提示已完成更改保留、核对后再预览。没有新增数据库 Schema、权限名或生产依赖。

必要公开配套包括 preview/export 可选 format、native 预览的配置及本地来源、逐项 updated 结果，以及 import 可选 overwriteExisting。原调用缺省保持旧导出，旧 TXT 结果不额外输出新增字段。Schema、协议、SDK、桥接和 [格式规范](../../specs/formats/repository-configuration-v1.md) 已同步；实施依据见 [计划](../exec-plans/M7-repository-configuration-files.md)。认证仅写入所有者确认的文件，不进入预览 RPC、日志或普通错误；保留原认证提示和新文件创建规则。

## 验证

不同 Rust 用例共 188 项通过，浏览器不同用例 53 项通过；最终修改后重复定向验证不重复计数。

| 范围 | 实际结果 | 记录 |
| --- | --- | --- |
| 配置与旧列表解析/导出 | 10 项；完整字段、内置/本地/认证、无关索引修订、畸形/未知版本/重复来源/重复开关、字节/条目边界及新文件保护 | target/m7-repository-config-vpm-final.log |
| 实际 Store/VPM/设置用例 | 25 项；其中 2 项原生配置回归最终再次通过，含 ID 重映射、默认覆盖、仅新增、其他项保留、无操作重复导入、权限、提前停止、预览变化、不覆盖、真正名称冲突与普通索引刷新区别 | target/m7-repository-config-integration-final.log、m7-repository-config-native-final.log |
| 实际 RPC 往返 | 1 项；旧 TXT 范围/认证语法回归及新增原生导出、预览、默认覆盖、名称和包开关恢复 | target/m7-repository-config-rpc-final.log |
| 公开协议 | 49 项；含原请求省略新增字段、native 配置及 updated、禁止预览凭据 | target/m7-repository-config-protocol-final.log |
| 应用层 | 80 项 | target/m7-repository-config-application.log |
| GUI 原生适配器 | 23 项 | target/m7-repository-config-build-20261110/gui-tests.log |
| 浏览器 | 53 项；仓库列表 33 项与两组选择器 20 项，默认完整导出、兼容 TXT、默认覆盖/取消覆盖、预览及部分结果/停止/未知错误等 | target/m7-repository-config-browser-final.log；共用视图失效补充后仓库列表定向复跑见 m7-repository-config-browser-complete.log |
| 静态与构建 | 受影响 Core 六个包及 GUI all-targets 严格 Clippy、fmt、xtask、全工作区前端检查及生产构建；独立 Core 和嵌入式 GUI 构建 | target/m7-repository-config-clippy-final.log、m7-repository-config-build-20261110/ |
| Schema | PowerShell Test-Json 验证格式示例及旧/新/overwrite 请求通过 | target/m7-repository-config-schema-final.log |

初轮严格 Clippy 的三处嵌套条件提示已整理并复跑通过。完整协议测试还发现上一轮主目录修改遗漏的单条当前向量断言仍期望 Documents，已同步为已批准的主目录路径，49 项通过；历史迁移向量保持。Python 额外 Schema 验证尝试因未安装 jsonschema 未执行，未新增依赖，改用系统已有 Test-Json 完成相同范围。原有大包构建提示及独立输出路径的 Cargo 提示保留，不影响构建结果。

补充共用视图失效的首次浏览器复跑中，发现旧 TXT 传输失败路径多触发一次刷新，已将新配置文件统一交给既有失效调度、旧 TXT 保持原刷新；同时一项未涉及导入的包详情返回列表用例偶发只展示一行。随后不改该用例或原断言，完整仓库列表 33 项再次全部通过，保留第一次失败的 trace/截图，不将失败轮次计为通过。

未运行全工作区 Rust、跨平台 Hosted CI 或原生系统选择器人工点击，本轮不扩大为 M7/H7 最终签收。之前记录的旧元数据脚本 Config12/13 不一致未在本项修改或放宽。

## 交付边界

保留此前全部工作区修改。验证仅使用隔离数据、假来源和假认证，不导出真实用户凭据，不导入或修改真实存储库、项目或备份。独立产物及源文件摘要位于 target/m7-repository-config-build-20261110/；运行中的程序未替换，已安装 Core/GUI 摘要仍与上一轮交付相同。未提交、推送或发布。

## 完整拼写补充

所有者随后要求 Repositories 与模板后缀一样采用完整拼写，最终扩展名统一为 `.alcomdrepositories`，默认文件名为 `repositories.alcomdrepositories`。同步产品元数据、协议常量、原生选择器、GUI 默认值、夹具/断言、Schema 和当前说明；文件内容仍是同一 JSON v1，旧 TXT 行为保持。受版本控制的文本中未再发现简称后缀。

本次定向重跑 8 项浏览器和 1 项实际 RPC 往返通过，fmt、xtask、GUI all-targets 检查、全工作区前端检查/生产构建及 Core/嵌入式 GUI 独立构建通过。最终产物、源码摘要及本轮日志位于 target/m7-repository-extension-20261110/，浏览器日志为 target/m7-repository-extension-browser.log；该目录替代上轮待更新产物。当前运行程序与用户数据保持，未提交或推送。
