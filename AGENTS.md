# AGENTS.md

## 必读顺序

开始任何任务前，按顺序阅读：

1. `docs/architecture/ALCOMD-V4.md`
2. `docs/decisions/accepted.md`
3. `docs/decisions/open.md`
4. 当前任务对应的 `docs/exec-plans/*.md`
5. `docs/status.md`
6. 当前目录下更具体的 `AGENTS.md` 或 `AGENTS.override.md`

旧版源码应位于 `../ALCOMD3-v3-readonly`，只能读取，不得修改。

## 不可违反的架构边界

- `alcomd` 是唯一状态持有者、数据库写入者和 Unity 项目写入者。
- GUI、CLI、MCP、Local API、外部应用与扩展不得直接访问数据库或修改项目。
- 所有入口必须调用同一个 `alcomd-application` 用例层。
- `alcomd-mcp` 必须独立于 GUI 和扩展运行。
- MCP 可视化管理界面必须作为第一方扩展实现。
- Discord GUI、后台逻辑和 Rich Presence 必须作为第一方扩展实现。
- 第一方扩展必须使用与第三方扩展相同的公开 API、权限、宿主和沙箱。
- 正常运行时代码不得依赖 v3、VCC 或旧 ALCOM 数据解析器。
- 旧解析器只能存在于 `migrations/v3/`，且不得进入正常安装产物。
- 内部技术身份永久使用 `ALCOMD`、`alcomd` 与 `com.cqmhv.alcomd`。
- 不得在生产代码中重新引入 `com.cqmhv.alcomd3`、`CQMHV.ALCOMD3`、`alcomd3-mcp` 或 `ALCOMD3.exe`。

## 工作范围

- 一次只完成当前 ExecPlan 中的一个里程碑。
- 不擅自扩大范围或提前实现后续里程碑。
- 不为了让测试通过而删除功能、降低校验、放宽权限或改写验收条件。
- 新增生产依赖前，必须在 ExecPlan 中记录用途、替代方案和维护风险。
- 不进行无关重构，不全仓库格式化无关文件。
- 公共 RPC、数据库 Schema、Extension API、权限名称和迁移删除清单必须经过人工审批。
- 不把占位实现描述为已完成功能。

## 代码规则

- Rust 和 TypeScript 使用 4 个半角空格缩进。
- 优先使用强类型与显式错误，不硬编码平台路径、产品名或旧标识。
- 产品身份从 `alcomd.product.toml` 派生。
- 公共 DTO 与领域对象分离。
- 不在 `alcomd-domain` 中依赖 Tauri、SQLite、HTTP、MCP 或操作系统 API。
- 禁止未经说明的 `unsafe`。
- 日志不得包含 token、Authorization 头、完整私密路径或项目敏感信息。

## 验证

修改 Rust 后运行适用的：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check
```

修改前端后运行适用的：

```text
npm run check
npm run build
```

修改协议或扩展契约后：

- 更新对应 Schema 与规范。
- 更新契约快照。
- 运行兼容性测试。
- 在 ExecPlan 中说明是否为破坏性变化。

无法执行验证时，必须记录原因、未覆盖范围和风险。每个里程碑完成后更新 `docs/status.md`。

## Git 与外部操作

- 不执行 push、force-push、release、tag 或远端仓库写操作。
- 不修改 GitHub 设置、Secrets、真实更新端点或真实签名材料。
- 不覆盖用户已有改动。
- 不修改旧只读仓库。
- Git 提交应由真实 Windows 用户环境执行，除非用户明确授权。
- 任何向第三方仓库写入的操作都需要用户明确授权。
