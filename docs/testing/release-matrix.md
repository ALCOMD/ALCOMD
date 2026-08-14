# 4.0.0 发布测试矩阵

状态：草案，平台范围待确认。

## 平台与安装

| 平台 | 安装方式 | 升级来源 | 必测 |
|---|---|---|---:|
| Windows x64 | 用户安装 | v3 stable | 是 |
| Windows x64 | 用户安装 | v3 beta | 是 |
| Windows x64 | 全局安装 | v3 stable | 是 |
| Windows x64 | 全局安装，多用户 | v3 stable | 是 |
| Windows x64 | 自定义路径 | v3 | 是 |
| macOS arm64 | App Bundle | v3 | 待确认 |
| Linux x64 | AppImage | v3 | 待确认 |
| Linux x64 | DEB | v3 | 待确认 |

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
- 冻结 vrc-get 与 v4 的依赖计划和安全错误。
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

1. 安装 v3。
2. 制造完整状态。
3. 快照文件、注册表、快捷方式、协议、凭据和进程。
4. 执行 v4 Bridge。
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
