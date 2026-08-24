# ADR: Extension ABI v1、Host topology 与生命周期 authority

- 状态：Accepted for M6 contract-first Stop A candidate；production pending
- 日期：2026-08-24

## 背景

ALCOMD 需要让第一方与第三方扩展共享公开 ABI，同时保持 daemon 唯一写入、权限即时撤销、恶意 guest 隔离和
可恢复 package lifecycle。WIT record 不是 JSON DTO；把多个不相干 ExtensionId 放进同一 Host 也会扩大 crash/
OOM 与 authority blast radius。

## 决策

1. ABI v1 使用 `alcomd:extension@1.0.0` versioned WIT world 与 WebAssembly Component Model。现有 type/function
   shape 的任何增删改默认 breaking；兼容扩展用新 optional interface/capability/world/version。
2. M6 第一条 slice 不链接任何 ambient WASI 0.2 interface。只 import scoped Project summary 与 bounded self data。
3. 每个 enabled ExtensionId 独占一个 Host OS process；第一条 slice 每 Host 一个 active Component instance。
4. daemon-created `ExtensionInstanceLease` 绑定 ExtensionId、InstanceId、PrincipalId、grant revision、lifecycle
   generation 和 expiry/cancel state。pipe session 而非 guest parameter 绑定 identity。
5. revoke grant revision commit 是 authority linearization point。已获 OperationId 的 core Operation 继续遵守既有
   recovery，但不获得新的 capability authority。
6. durable desired state、quarantine enforcement 与 runtime process state 三者分离；`enabled + quarantined + stopped`
   合法，quarantine 不覆盖用户 intent，数据库 enabled 不等于进程存在。
7. extension-owned data v1 只有 bounded opaque-byte key/value `get/set/delete`；namespace 绑定 ExtensionId + publisher
   fingerprint。uninstall 默认保留，显式 high-impact immutable Plan 才可删除；grant 永不作为 reinstall authority 保留。
8. package content identity、publisher cryptographic identity、local trust 与 first-party policy 分层。Manifest
   不能自报 first-party；unknown/self-signed approval 只绑定 ExtensionId + fingerprint + package digest。

## 结果

- Host pool、通用 DI/service discovery/workflow、native extension、raw socket/arbitrary filesystem 被排除。
- M6 只冻结 UI Bridge security envelope/headless harness；M7 才冻结产品 placement。
- UI origin 在 M6 是 ExtensionId + package digest logical identity；具体 custom URL scheme 留给 M7。
- 项目所有者已在 Stop A review 批准 Wasmtime 48.0.0 与 ed25519-dalek 3.0.0 的精确最小配置；其他 production
  dependency 仍需单独审批。
