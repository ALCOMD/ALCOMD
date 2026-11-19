# ADR: Discord 第一方扩展

- 状态：Accepted
- 日期：2026-08-14
- 修订：2026-11-20

## 背景

Discord 是第一方扩展，同时作为组织维护的第三方扩展开发示例。示例需要能从独立仓库通过公开合同构建、安装和运行。


## 决策

扩展显示名为 **ALCOMD Discord Presence**，计划仓库为 `ALCOMD/alcomd-discord-presence`，包 ID 为 `com.vrclearn.alcomd-discord-presence`。设置、隐私过滤、Discord IPC 协议、连接与重连、Rich Presence 更新和清除，以及扩展的后台业务都由该仓库实现。

ALCOMD 主仓维护项目与 Unity 的通用用例、公开扩展 API、权限和生命周期。Core 和 Extension Host 的接口保持应用无关。该扩展经由同一公开接口、包校验、权限及沙箱运行，适用与第三方扩展相同的规则。

## 结果

M9 先核对当前公开接口。确有缺口时，单独提交最小通用能力的合同供审批。任何本地通信能力都需要按扩展及明确目标授权，并由宿主管理句柄的生命周期。Discord 协议及应用侧行为由扩展实现。

独立仓库、现有接口缺口和验收顺序见 [M9 ExecPlan](../exec-plans/M9-discord-first-party-extension.md)。
