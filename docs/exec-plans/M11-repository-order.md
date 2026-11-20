# M11 仓库顺序与来源优先级

## 目标与范围

让仓库列表从上到下决定同版本仓库来源的优先级。官方和精选固定在前。普通本地、远程仓库可以拖拽或通过行菜单移动。
在 `repository-order` 工作树完成仓库相关的 Core、Store、协议、客户端、GUI、导入导出及验证。
沿用 [Core 架构](../architecture/ALCOMD-V4.md) 的状态边界，保持既有数据库结构、迁移边界、来源资格及版本比较。
部署、发行、远端写入和真实数据更改各按项目既有授权执行。

## 已批准的合同

- [原生 RPC](../../specs/rpc/alcomd-rpc-v1.md) 新增 `repositories.move`。使用现有 `repositories.registry.v1` 与 `repositories.manage`，写入限本机所有者。指纹在 SQLite 事务内校验，写入与幂等回执一起提交。
- 原生仓库列表返回顺序指纹。原生和 MCP 游标都绑定该顺序，旧游标返回冲突。导入预览移除 `priority`，属于测试期客户端的破坏性合同变化。
- [专用格式 v1](../../specs/formats/repository-configuration-v1.md) 以数组顺序表达排序，严格拒绝未知字段。`priority` 从文件移除，旧测试期导出文件会校验失败。VCC TXT 沿用现有语法和来源范围。

## 实施与验证

1. 用同一 Core 顺序服务仓库列表、导出、候选和实际解析。调序和配置导入按预期的顺序提交。
2. 接通原生 RPC、SDK、分页及 GUI。拖拽和键盘菜单共用移动操作，保存后以 Core 读回为准。
3. 验证固定位置、同版本来源选择、关闭与重新启用、重启持久化、分页冲突、幂等性、两种导入策略及未知字段拒绝。

验证命令包括 `cargo fmt --all -- --check`、定向 `cargo clippy`、`cargo xtask check`、
`cargo test -p alcomd-testing --test m7_vpm_semantics --test m7_repository_list_rpc`、
`cargo test -p alcomd-protocol --test schema_contract`、`npm run check`、`npm run build`
及 `npm run test:browser --workspace @alcomd/gui -- m7-repository-list.spec.ts`。

## 风险与回滚

调序后包候选和计划可能选中新的同版本来源。用户可恢复原列表顺序。测试期文件字段及分页游标和对应客户端需一起更新。
数据库沿用现有排序列，应用回滚无需结构迁移。

## 进度记录

- 2026-11-21，所有者批准排序、v1 文件和公共 RPC 差异。工作树从本地 `main` 的 `cdbecce5` 建立。该提交是 M10 验收记录，主工作树中尚未提交的 M11 基线由原工作树保留。排序改动使用现有列，本工作树的验证以该独立基线为准。

- Store 单元 153 项通过，VPM 单元 192 项通过、3 项沿用原忽略条件，RPC/SDK Schema 合同 54 项通过。VPM 集成 26 项通过，其中调序回归验证顺序 CAS、游标失效、幂等重试、无变化操作、关闭和重新启用、独立数据库重开，以及同版本来源切换后的实际安装。
- 前端检查及构建、Rust 格式检查、受影响 crate 的严格 Clippy 与 `cargo xtask check` 通过。相关 Chromium 三模块一轮 81 项通过、1 项菜单禁用断言失败。改用 Material 元素的 `disabled` 属性后，拖拽与键盘焦点、冲突读回两项在独立轮次通过。
- Windows 真 IPC 用例两次在创建隔离 daemon 时返回 `AlreadyRunning`，同一用户的单实例锁仍被占用，未进入用例。真实 daemon 重启读回待复验，独立数据库重开已覆盖持久化。验证期间 `main` 前进到 M11 首发运行时基线 `98a8ed92`。已核对该提交的 State46 仍保留排序列。当前工作树的验证绑定创建时的 `cdbecce5`，M11 合并后的验证及 Hosted CI 待执行。
