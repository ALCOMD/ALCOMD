# v3 到 v4 迁移规范

状态：Draft

## 核心不变量

- v4 迁移链只接受 ALCOMD3 3.4.0 v3 迁移入口版本；更早的 v3.x 必须先升级到 3.4.0。
- 3.4.0 必须从 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 或 `https://alcomd.cqmhv.com/api/v1/updates/beta.json` 获取并验证 v4 迁移桥接安装器（v4 bridge installer）元数据。
- Commit 前不得破坏 v3。
- Health Check 前不得删除旧资源。
- Cleanup 只能删除已确认由 ALCOMD3 拥有的资源。
- 用户自定义项目、备份和其他应用配置不得猜测删除。
- 成功后不得保留永久迁移标记或旧路径回退。

不支持的源版本必须在 Inventory 阶段安全拒绝，并明确提示用户先升级到 3.4.0。拒绝不得修改 v3 状态或创建可被误认为有效的 v4 状态。

## 阶段

```text
Inventory
Freeze
Export
Validate Export
Build Native State
Move Owned Folders
Install New Identity
Install First-party Extensions
Health Check
Commit
Cleanup
Residue Audit
```

## Commit Point

进入 Commit 前必须满足：

- 新程序文件完整。
- `state.db` 与 `settings.toml` 可加载。
- RPC、GUI、CLI 和 MCP 基础健康检查通过。
- 项目、仓库、模板和设置数量匹配。
- 第一方扩展签名与加载通过。
- 回滚所需 v3 资源仍存在。

## 零残留

```text
migrated_state - (fresh_v4_state + equivalent_user_data) = empty
```

Residue Audit 的对象来自 `migrations/v3/artifacts.toml`，不得在代码中散落硬编码删除列表。
