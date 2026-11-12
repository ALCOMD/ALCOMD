# Core 空闲自动退出验证

依据[实施计划](../exec-plans/M7-core-idle-shutdown.md)，基线 `bc00831a`。2026-11-12 所有者明确授权在所有客户端连接关闭、没有未完成任务时自动退出。

## 实现与边界

Core 主循环统一跟踪普通 RPC 和事件订阅的连接处理任务，不按 GUI/CLI 名称或自报身份猜测。最后一个连接结束后留1秒交接，新连接优先于空闲判断且会使原判断失效；尚无首次连接的启动留5秒。经 application 访问 Store 单写入队列，以全局 EXISTS 查询判断非终态任务，不使用当前调用者过滤后的活动计数。

只有 succeeded、failed、cancelled 算完成，其他七种状态均保留 Core。查询失败保持运行并有界记录诊断，不把未知状态视为没有任务。在途初始同步、发现和自动刷新收尾后再退出；后台扩展租约不单独常驻，退出正常停止宿主。未新增 RPC、DTO、Schema、权限或依赖。架构及 RPC 生命周期说明同步；未来 MCP/API 须在服务客户端期间保持 IPC，上游全部关闭后释放相应连接，本轮不实现这些适配器。

## 测试

日志位于 `target/core-idle-*.log`。共107项不同 Rust 用例通过：

- Store 全库94项（`core-idle-store-closure.log`），包含新增的全局任务/全部十种状态矩阵及不可用 Store 错误传播两项。矩阵使用其他 Principal 的任务，确认本地所有者列表为空仍不能退出。
- Core 启动/归属/生命周期10项：`core-idle-daemon-closure.log` 的9项，加 `core-idle-startup-sync.log` 的1项。其中6项新增真实当前用户 Named Pipe 回归，覆盖首次连接、多连接关闭、事件流单独存活、交接重连、无客户端的持久任务经真实恢复调度继续执行至成功、无首次客户端时退出、在途初始同步及退出后端点释放/再次启动。
- M1 RPC 与 M2 state.check 两项真实 IPC（`core-idle-rpc.log`），CLI 一项真实子进程（`core-idle-cli.log`），保留握手、单实例、任务、错误及 CLI 输出约定。

受影响 `alcomd`、`alcomd-application`、`alcomd-store` 的 all-targets/test-kill-gates 严格 Clippy 通过（`core-idle-clippy-final.log`）。首轮 Clippy 指出新增测试使用 unwrap，改为明确的 expect 说明，未降低 lint、业务判断或断言；原失败保留在 `core-idle-clippy.log`。修正后新增八项回归再次全部通过（`core-idle-daemon-final.log`、`core-idle-store-final.log`），复用未改变的其余99项结果。最终 fmt、xtask 和差异检查通过（`core-idle-fmt-final.log`、`core-idle-xtask-final.log`），`core-idle-inputs.json` 保存收尾输入摘要。

## 限制与交付

测试使用实际 Windows 当前用户 IPC 和隔离临时数据库/项目目录。测试夹具在端点已占用时失败，不关闭现有 Core，不访问真实项目或应用用户数据。未验证 Linux/macOS 运行、三平台 Hosted CI/CodeQL、正式安装器或尚未实现的 MCP/API；GUI 未改，未扩展前端/浏览器测试。

实施验证阶段未更新或重启真实 Core/GUI；所有者随后授权本项提交并更新程序，实际交付另记，未推送或发布。本项不扩大 M7/H7 整体验收。

功能随后提交98e6b95并完成[运行更新](core-idle-runtime-update-20261112.md)。原待恢复删除任务仍非终态，真实Core正确保持运行；本轮没有将其强制改为终态来验证退出。隔离回归与真实运行验证的范围分别保留。
