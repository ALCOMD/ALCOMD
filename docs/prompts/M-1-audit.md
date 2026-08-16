/plan

这是 ALCOMD v4 的 M-1 规划与审计任务，不是实现任务。

开始前必须阅读：

1. AGENTS.md
2. docs/architecture/ALCOMD-V4.md
3. PLANS.md
4. docs/exec-plans/M-1-audit.md
5. docs/status.md

旧版只读源码位于 ../ALCOMD3-v3-readonly。
vrc-get 功能与行为基线仓库位于 ../vrc-get-readonly，只能用于只读功能审计、行为观察、安全分析和差异测试。

不得修改旧版源码，不得执行远端 Git 写操作，不得新增生产依赖，不得修改 apps/、crates/、extensions/ 或 packages/。
不得复制、移植或改写 v3 源码；不得复制、Fork、包装、移植或改写 vrc-get / vrc-get-vpm 实现源码。

请使用适合的只读 Subagent 分别审计：

1. ALCOMD3 v3 的全部用户功能和隐性行为
2. 固定版本 vrc-get 的全部功能、安全行为、CLI、错误处理，以及公开 VPM 格式、生态兼容行为与独立实现边界
3. 安装器、更新器、数据路径和 v3 残留
4. RPC、权限、扩展和第一方扩展边界
5. Windows、Linux、macOS 构建与测试要求
6. 当前 MCP 规范与 alcomd-mcp 的兼容要求

最终只提交 M-1 ExecPlan 允许的规划与审计产物。

要求：

- 所有结论标明源码依据和文件位置。
- 不确定内容明确标记，不得猜测为已确认事实。
- 将产品决策与技术决策分开。
- 不得把占位代码描述为已实现。
- 完成文档后停止，不开始 M0 或生产实现。
