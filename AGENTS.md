# ALCOMD 项目规则

## 工作范围与授权

- 在项目所有者创建的 `C:\Users\M2922\MyProjects\Rust\ALCOMD` 的 `main` 上开发、检查、构建和提交。新增或切换 worktree、转到同级目录开发须另获项目所有者批准。
- 一次只推进当前 ExecPlan 内已授权的一个里程碑。公共 RPC、数据库 Schema、Extension API、权限名称和迁移删除清单须经人工审批。新增生产依赖前在对应 ExecPlan 记录用途、替代方案和维护风险。
- 本项目禁止 push、force-push、release、tag 和远端写入。只有项目所有者明确针对该操作授权例外时，才按授权范围执行。提交由真实 Windows 用户环境执行，除非所有者明确授权其他方式。禁止修改 GitHub 设置、Secrets、真实更新端点或签名材料。第三方仓库的写入也须有明确授权。
- `../ALCOMD3-v3-readonly` 和 vrc-get 参考仓库只读。v3 仅用于功能审计、迁移格式验证和兼容性测试。禁止复制、移植或改写 v3 源码，禁止复制、Fork、包装、移植或改写 vrc-get / vrc-get-vpm 源码。

## 按任务读取

- 需要判断当前实施范围或验收状态时，先读 [项目状态](docs/status.md)，再读其中对应任务的 ExecPlan 章节。触及子目录时先查看适用的 `AGENTS.md` 或 `AGENTS.override.md`。单纯查证、文案或局部样式改动，只读相关规则、代码和验证入口。
- 进程、依赖或模块边界查 [架构](docs/architecture/ALCOMD-V4.md) 的对应章节。决策依据和待决事项分别查 [已接受决策](docs/decisions/accepted.md) 和 [开放决策](docs/decisions/open.md)。修改公共合同再查对应 `specs/`。对接 VPM 等规范时先核对官方来源并遵循其语义，发现与既有合同冲突时记录差异，按原审批边界修订。
- `docs/status.md` 是唯一当前阶段入口，ExecPlan 定义实施合同。`docs/testing/` 和 `codex/archive/*` 用于查历史证据。工作区整合记录见 [保留与恢复记录](docs/testing/workspace-consolidation-20260917.md)。

## 架构与代码

- `alcomd` 是唯一权威状态持有者、数据库和 Unity 项目写入者。GUI、CLI、MCP、Local API、外部应用和扩展统一通过 `alcomd-application` 用例，不直接访问数据库或写项目。业务校验、资格与范围、调度、冲突恢复和结果由 Core 实现，GUI 只负责交互、呈现和本地视图状态。
- `alcomd-mcp` 独立于 GUI 和扩展。MCP 管理界面与全部 Discord 能力是第一方扩展，遵循与第三方相同的公开 API、权限、宿主和沙箱。
- 正常运行时代码与安装产物排除旧解析器。v3、VCC 和旧 ALCOM 数据解析器只留在 `migrations/v3/`。
- 从 `alcomd.product.toml` 派生产品身份。永久技术身份是 `ALCOMD`、`alcomd`、`com.vrclearn.alcomd`、`VRCLearn`、`vrclearn.ALCOMD`。生产代码不得引入 `com.cqmhv.*`、`CQMHV`、`CQMHV.ALCOMD*`、`alcomd3-mcp` 或 `ALCOMD3.exe`。迁移输入与历史证据保留真实旧值。
- Rust 与 TypeScript 用 4 空格缩进。优先强类型和显式错误，公共 DTO 与领域对象分离，不硬编码平台路径、产品名或旧标识。`alcomd-domain` 不依赖 Tauri、SQLite、HTTP、MCP 或 OS API。`unsafe` 须说明。
- 获批的日志入口可记录排障所需的绝对路径和技术细节，禁止记录 secret。RPC、业务事件及普通 GUI 错误保持有界稳定合同。

## 验证与状态

- 日常运行受影响模块的定向测试、fmt/Clippy 和前端检查。功能模块完成后补相关回归，跨平台、安全、依赖与架构变更覆盖相应边界。稳定候选、阶段收尾和最终验收集中执行适用门禁：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo xtask check`、`npm run check`、`npm run build`、浏览器回归与三平台 Hosted CI/CodeQL。复用聚合脚本已有结果，最终验收绑定同一候选 SHA。
- 保留现有功能、断言和安全门禁。不得为使测试通过而缩减功能、放宽校验或权限、改写验收条件。获批修改协议或扩展契约后，同步 Schema、规范和快照，运行兼容性测试，并在 ExecPlan 写明是否为破坏性变化。
- 区分通过、失败和未执行，记录未覆盖的原因、范围和风险。占位实现不能计为完成。里程碑完成后更新 `docs/status.md`。
