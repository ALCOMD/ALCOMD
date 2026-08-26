# M7 current Official GUI user-model gap

审计基线：本地 functional candidate `19267230507071dc61ba306b98c8cfdd113e9ea2`。

结论：该候选的 Core/RPC、Settings Config v1、Activity、Diagnostics、Portable UI、Plan/Apply、Playwright 和 accessibility
基础设施继续作为有效功能证据；但 visual architecture 与 information architecture 未通过项目所有者验收：

```text
functional_implementation_candidate
but
rejected_for_visual_and_information_architecture_acceptance
```

该候选不得 push，也不能进入原 manual checklist。它不需要业务回滚；后续 minimum-change realignment 复用 typed RPC、hooks/state、
workflows、Settings、Activity、Diagnostics、Portable UI 和 Playwright infrastructure，只重构 presentation/composition/component
layer。

## Root cause

当前候选把 M1-M7 的 implementation inventory 近似直接投影成 GUI navigation：Home、Projects、Repositories、Templates、Unity、
Operations、Extensions、Activity、Diagnostics、Settings、About 全部平级。其核心错误不是“与 v3 每一项不同”，而是把 daemon/
RPC/domain/State 边界误当作用户拥有一整套新产品领域。

用户仍主要是在管理 Projects、packages/resources、Extensions、Settings 与 logs。Plan/Apply、OperationId、Extension Host、Portable
UI、permission/scope 和 diagnostics read model 应优先增强这些既有任务。

## Surface audit

| Surface | Current candidate | User-model finding | Minimum-change correction |
|---|---|---|---|
| Main shell | 长期 top brand/settings bar + 11-item flat rail + generic content | 第一眼不再是 ALCOMD3 desktop tool；全局hierarchy竞争 | 恢复 sidebar + single content canvas；page title/action回到main toolbar |
| Home | status cards作为默认landing | daemon/RPC status不是新产品领域 | Projects恢复home-equivalent；connection进入shell/footer；不保留独立domain dashboard |
| Projects | generic title/cards/page-bottom actions | v3既有用户区域被低密度化 | 恢复toolbar、search、refresh、list/grid、create/import/restore与dense content |
| Project detail | generic detail + Packages/Unity/Backups button subnav + dispersed actions | package-centric workflow与context action关系被削弱 | Packages为主要workspace；Unity/open/backup/copy/remove回到header、secondary view或overflow |
| Repositories | 独立primary route和generic cards | 既有Packages & Templates子能力被错误拆分 | 移回资源区域的Repositories secondary view；保留route作deep link |
| Templates | 独立primary route和generic cards/forms | 同上 | 移回资源区域的Templates secondary view；create-project连接Projects workflow |
| User Packages | 完整能力未实现 | v3已有概念，但v4不能显示fake surface | 等真实capability完成后在同一资源区域恢复 |
| Unity | global primary route + project subroute | global configuration和project action被当成独立domain | global installations移回Settings；project selector/open/writer state留在Project |
| Backups | Project tab/detail | 用户能力是Project create/restore；durability只是增强 | 保留Project-context history/detail；主动作从Projects/Project发起，不新增global nav |
| Operations | 独立primary list/detail | durable history是progress/task增强，不是独立业务领域 | context progress + footer active-task；Task Center/history作为secondary utility |
| Extensions | 与所有domain平级的generic list/detail | Extensions本来就是v3用户概念，但归属被flat nav稀释 | 保留既有utility身份/大致归属；permission/quarantine/Portable UI进入detail |
| Activity | 独立primary page | v3 Log/Activity的增强 | 合并进一个Log/Activity utility的Activity secondary view |
| Diagnostics | 独立primary page | structured/redacted Technical Log增强 | 同一utility的Diagnostics secondary view；RPC/permission继续独立 |
| Settings | top shortcut + sparse generic sections | 既有utility hierarchy和grouping被削弱 | grouped scrolling Settings；appearance与global Unity配置归入相关section |
| About | 独立primary大页面 | v3已有footer/settings utility概念 | 移回version/footer action或Settings secondary page |
| Dialog/controls | native HTML + custom CSS Material-like controls | Material component foundation尚未采用 | 迁移到共享`@alcomd/ui` / Material Web；不改变authority |
| Data density | generic large cards、巨幅标题、大量留白 | 不符合desktop management tool与v3识别性 | packages/repositories/projects/operations/extensions/logs优先dense list/table/compact row |
| Narrow/a11y | drawer/focus/320 px logic存在 | functional evidence可复用 | 将批准后的同一语义层级adaptive映射到drawer/compact tabs/priority columns |

## Navigation splits to merge, move or remove

- **remove as primary**：Home、Unity、Operations、Diagnostics、About；
- **merge into Packages & Templates**：Repositories、Templates，未来真实 User Packages；
- **move into Project context**：project Unity、Backups、Project Packages；
- **move into Settings**：global Unity installations、appearance与其他全局配置；
- **merge into Log utility**：Activity、Diagnostics；
- **retain existing concept but restore utility grouping**：Extensions；
- **retain as deep links/secondary routes**：上述 route identity在不出现在sidebar时仍可用于context/history/detail。

## Evidence retained

- typed Tauri adapter、`alcomd-client`、RPC 与 application authority；
- route/deep-link identity 和 opaque resource ID；
- Core reads/mutations、Plan review、Apply、Operation follow/cancel/reconnect；
- Settings Config v1、Activity 与 redacted Diagnostics contracts；
- Portable UI protocol/session/action/host-owned chrome；
- dirty/discard、loading/refreshing/empty/error/disconnected state logic；
- Playwright 1.62.1 keyboard/focus/ARIA/contrast/320 px/reduced-motion infrastructure。

这些证据不授权继续使用当前 flat IA、custom Material imitation 或低密度 composition。
