# 4.0.0 发布测试矩阵

状态：草案，平台范围待确认。

## 平台与安装

| 平台 | 安装方式 | 升级来源 | 必测 |
|---|---|---|---:|
| Windows x64 | 用户安装 | ALCOMD3 3.4.0（v3 迁移入口版本） | 是 |
| Windows x64 | 全局安装 | ALCOMD3 3.4.0（v3 迁移入口版本） | 是 |
| Windows x64 | 全局安装，多用户 | ALCOMD3 3.4.0（v3 迁移入口版本） | 是 |
| Windows x64 | 自定义路径 | ALCOMD3 3.4.0（v3 迁移入口版本） | 是 |
| macOS arm64 | App Bundle | ALCOMD3 3.4.0（v3 迁移入口版本） | 待确认 |
| Linux x64 | AppImage | ALCOMD3 3.4.0（v3 迁移入口版本） | 待确认 |
| Linux x64 | DEB | ALCOMD3 3.4.0（v3 迁移入口版本） | 待确认 |

## 迁移入口与更新源

- ALCOMD3 3.4.0 是 v3 迁移入口版本（v3 migration entry release）；v4 迁移链只接受它作为直接迁移来源。
- 所有更早的公开 v3.x 必须先通过原有更新链升级到 3.4.0。
- 3.4.0 已把更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json`。
- 上述已上线路径和频道映射作为测试基线；v4 迁移桥接安装器（v4 bridge installer）的 JSON Schema、版本推进、签名验证和错误处理必须形成冻结契约。
- 不支持的旧 v3.x 直接启动 v4 迁移时，必须安全拒绝并明确提示先升级到 3.4.0，不得尝试猜测解析旧状态。

必须覆盖以下更新链：

1. 每个受支持的旧 v3 版本通过旧更新源升级到 3.4.0。
2. 3.4.0 从新标准 API 发现并验证 v4 迁移桥接安装器。
3. 3.4.0 启动 v4 迁移桥接安装器，该安装器再启动 `alcomd-bootstrap` 和临时 `alcomd-migrate-v3`。
4. 新 API 不可用、返回无效 JSON、频道不匹配或签名失败时，3.4.0 保持可恢复且不得进入迁移。

## 用户目录

- 默认 Documents。
- OneDrive 重定向 Documents。
- 非 ASCII 用户名与路径。
- 自定义项目目录。
- 自定义备份目录。
- 目标 `ALCOMD` 目录预先存在。
- 文件被占用。
- 磁盘空间不足。

## 功能对照

- v3 与 v4 对相同项目副本的最终结果。
- 用公开 VPM 格式、生态 Fixture 与 v4 自有预期验证依赖计划和安全错误。
- GUI、CLI、MCP、RPC 和扩展入口调用同一用例。
- JSON 输出和错误码契约。

## 并发

同时运行：

```text
1 个 GUI
2 个 CLI
多个 MCP 客户端
1 个 API 客户端
多个扩展
```

验证同项目写入串行、不同项目并行、取消和断线恢复。

## 扩展

- 第一方与第三方使用同一 API。
- UI 沙箱。
- WASM 崩溃隔离。
- 权限撤销立即生效。
- 禁用 MCP 管理扩展不影响 MCP。
- 禁用 Discord 扩展清除 Presence。
- 扩展卸载无后台残留。

## 迁移与零残留

每个 Fixture：

1. 安装 ALCOMD3 3.4.0，或从受支持的更早 v3 版本升级到 3.4.0。
2. 制造完整状态。
3. 快照文件、注册表、快捷方式、协议、凭据和进程。
4. 通过 3.4.0 的新标准 API 更新源执行 v4 迁移桥接安装器。
5. 验证功能和数据。
6. 执行 residue audit。
7. 与全新 v4 + 相同用户数据快照比较。

## 故障注入

- 每个迁移阶段终止进程。
- 系统重启。
- 数据库提交失败。
- 网络中断。
- 包校验失败。
- 更新签名失败。
- Extension Host 崩溃。
- daemon 重启。
