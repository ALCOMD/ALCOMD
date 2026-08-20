# ADR 0019：M5 Unity registry、启动与外部 writer gate

状态：Accepted（2026-08-20，contract-first；生产实现尚未获批）

## 决策

M5 在 State Schema v4 增加最小 Unity installation registry 与 project Editor preference。manual
registration 和 automatic discovery 必须经过同一个 executable validator；Hub config、known install
root 与 Unity CLI 只能提供 advisory candidate，最终记录必须对应本机真实、可验证的 Editor
executable。deprecated Hub CLI 不成为依赖或权威来源。

installation identity 来自现有平台 filesystem identity；path、显示名称和版本不是权威身份。记录
包含 installationId、验证后的 executable path、opaque filesystem identity、Unity version、
architecture（含诚实的 unknown）、source kind、revision 与观察/更新时间。project preference 只保存
ProjectId、InstallationId 和最多 64 项的 argv 数组；不保存 shell command。

启动固定为 safe API 等价的：

```text
Command::new(validated_editor)
    .arg("-projectPath")
    .arg(absolute_project_root)
    .args(validated_user_arguments)
```

不得通过 shell。用户参数拒绝 `-projectPath` 及大小写/前缀等价的重复 project selector。spawn accepted
只产生 `opening` launch record，不代表项目已经打开；后续 observation 才可变为 `open`/`failed`。
foreground/activation 不是本 slice blocker，未获批平台 API 时明确 unsupported。

## Writer state 与 gate

公共状态固定为：

- `running_confirmed`：正向 process/project evidence 确认目标正在被 Unity 使用。
- `running_suspected`：例如有 project lock evidence，但不足以确认 process mapping。
- `not_observed`：当前平台可执行的检查已完成且未观察到实例；不表示 definitely not running。
- `unknown`：权限、平台或系统错误使检查无法可靠完成。

对 package mutation、从现有项目生成 template、需要稳定 snapshot 的 backup create：confirmed
返回 `unity_project_running`；suspected/unknown 产生 advisory 并继续依赖 live fingerprint 与
changed-during-operation gate；not_observed 正常继续。不存在通用 `--force`。

对 Unity launch：confirmed 拒绝第二实例；suspected/unknown 返回
`unity_launch_state_uncertain`；只有 not_observed 允许 spawn。ALCOMD Resource Lock 只能协调 ALCOMD
writer，不能宣称约束任意 Unity/外部进程。

## RPC 与权限

RPC v1 兼容增加 `unity.read.v1`、`unity.manage.v1`、`unity.launch.v1`，method/DTO 由
`specs/rpc/m5-unity.schema.json` 冻结。`unity.read` 只查询 registry/writer state；`unity.manage`
管理 installation、refresh 与 project preference，不包含 launch；`unity.launch` 只允许启动/观察
已验证 Editor。capability、path、PID、launchId 与 client metadata 都不是身份凭据。

Schema/permission/RPC 合同可以先落盘；在生产实现、进程发现依赖/平台 API 与测试获得后续批准前，
daemon 保持 Data Schema v3，不执行 0004 migration、不广告 M5 capability，也不接受这些 method。

## 结果与限制

进程检查失败必须映射为 unknown，不能伪装成 not_observed。PID reuse 必须通过 executable identity 与
start evidence 防护；实现不能只按进程名判断。Hosted CI 使用 fake executable/process provider 证明
工程合同，不冒充真实 Unity/Hub differential parity。真实 v3/Unity evidence 继续 blocked 到 M11。
