# M7 H2-A：短请求的瞬时反馈清理

## 当前行为

按所有者要求，瞬间存在的请求状态不增加可见文字或临时行。主题 Save 始终显示原名称，
不显示 Saving…、不预留其宽度；禁用、同步重入锁、form aria-busy、明确错误及显式重试保留。
独立主题预览／Save／Cancel 和普通设置即时保存继续使用已有串行 CAS 协调。
Theme saved 是保存完成后持续保留的结果，直到下次打开编辑器，不属于一闪即逝的请求提示。

审计官方 GUI 的短请求按钮、列表分页、筛选、包候选、创建／恢复、仓库、模板、启动、
扩展及 Operation 跟随；扩展提供的 Portable UI 内容保持公开合同，不重写其状态文本。

- 删除筛选 Saving filters…、包 Checking package changes… 等临时行。
- Review、Apply、Confirm、Add repository、Register、Copy、Remove、Open Unity 和打开目录等动作
  请求期间保持原动作名称。保留现有 disabled、请求 guard、错误及真实 Plan Review。
- 分页保留既有条目、游标、重复请求锁和迟到响应隔离；加载时只禁用 Load more，不新增加载行。
  分页失败和 Retry 直到成功才清除，避免重试开始时折叠错误区。
- 首次读取复用 LoadingState 已有的 160ms 延迟；快请求不闪提示，慢请求仍可见，结果立即呈现，
  不新增最短展示时长。创建／恢复读取前不误判为空，也不先打开瞬时加载弹窗再替换为表单。
  包版本和候选证据首次读取、模板来源及启动参数使用同一规则；已有条目的翻页不再插入加载行。
- 保留真实长任务 Operation 的进度、阶段、取消、实际失败、来源过期／不可用原因和持续可读结果。
  不以候选 eligible 替代 Plan，也不改变 resolver、来源、配额、Plan／Apply 或快照权威。
- 清理不再使用的英／中／日瞬时文案；没有 CSS、图标、布局方向、依赖或生产 Rust 改动。

## 验证

证据目录：`target/m7-quiet-feedback-20261030/`。浏览器为既有隔离 fixture，未操作真实工程。

首轮 31/32：主题几何／防重／失败／冲突、分页及创建／恢复流程通过；新 Plan 测试把
Material Shadow DOM 原生按钮的空壳 textContent 当作可见标签，改用真实可访问名称，保留
忙时禁用、原名称、无 Planning…、原 dialog 四维边界变化不超过 1px 和真实 Review 结果断言。

修正后的专项／共享控件组 18/19：五项短请求专项全部通过，唯一失败为旧菜单危险色写死
#b3261e，而当前动态主题正确提供 #ba1a1a。断言改为必须等于有效 --md-sys-color-error 且 token
非空，不改产品颜色，也不降低动作危险色要求。其余共享控件、复制／删除／取消流程均通过。

随后工具栏复核越过危险色断言后，遇到旧 writer 状态断言仍要求原机器值 not_observed；
当前界面早已显示本地化的 not observed。仅同步该精确展示断言，保留同一 not-observed 语义，
最终工具栏复核 **1/1**（toolbar-recheck.log，退出码 0）。各轮日志均保留，不合并为同轮全通过。

最终相关证据：

- 五项短请求专项 **5/5**：快读取无闪烁／假空状态，慢读取可见，Plan 挂起时原名称、禁用和几何稳定。
- 首轮中的主题 **21/21**、分页 **3/3**、创建／恢复 **3/3**：明确保存／取消、一次提交、失败及 CAS，
  挂起保存的静态 Save／form aria-busy 和 dialog／footer 四维边界不超过 1px，分页重试／迟到响应。
- 候选／版本／批量 **17/17**（candidates.log，退出码 0）：来源切换失效、版本菜单、一次 Plan、
  完整选择集、配额及请求边界、真实 Review／Apply 流程继续保持。
- 最终 npm run check、npm run build、合同 JSON 解析、diff check 通过；三语词条边界 819 条，
  构建保留既有大 chunk 警告。无新增依赖，也未为本轮重复完整 Rust Workspace。
- localhost:1420 的模块 HTTP 200，ThemeControls 中无 Saving…，CorePages 提供延迟候选状态。
  只读真实进程确认 GUI PID 59180、隔离 daemon PID 59348 仍来自本工作区 target/debug；未启动
  或关闭其他进程。该证据不等于真实桌面视觉／屏幕阅读器验收。


## 边界

HEAD `7d938cf2845cd982c25b12f106a8dac33981c8e7`，继续唯一工作区 main，保留原有 WIP。
本轮不提交／推送，不重建原生程序或重复 Rust Workspace。前次普通设置记录的两项列表长文本
行高失败仍未闭合；本次不是完整 GUI suite，也未取得新增真实桌面／屏幕阅读器签收。
Visual Gate 2 继续 PENDING。
