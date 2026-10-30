# M7 H2-A：普通设置即时保存

普通设置按所有者要求采用 v3 用户语义：修改即保存，移除整组 Save settings／Discard changes／
All changes saved 及其旧样式、无用文案和普通草稿逻辑。主题等已有独立 Save／Cancel 的编辑器
保留明确确认；不把主题调色变成自动保存。只读审计 v3 settings/index.tsx 及普通控件的 mutation
触发方式，未复制源码。

## 当前行为与边界

- 语言、密度、动画、预发布显示、本地用户包显示、隐藏仓库选择均从对应用户动作触发，
  只发送本次变更的字段，经现有 settings.update expectedRevision CAS 保存。
- 当前 App／连接持有一个临时 UI 协调器，普通写入与主题确认共用串行队列；正在写入时，
  后续变更按字段保留最新意图，不并行写、不用旧整份配置覆盖其他字段，不靠任意延时防抖。
- 运行时语言／密度／动画只应用 Core 已接受结果。普通未确认意图保留在控件，成功后归于
  新 authority；正常导航不再出现普通设置的放弃确认，队列与失败信息不随页面卸载丢失。
- 实际失败在顶层显示错误和 Retry，不伪报成功。Retry 先 settings.get，重算仍需提交的
  字段，未知结果若已提交则不再重复写；保留其他字段，显式重试保留的同字段意图。
- 连接代次变化停止旧队列，不把旧意图自动重放到新连接。await 边界后再次检查 stopped，
  禁止在已停止连接上启动新的主题或普通 RPC；已经发出的 Core 请求仍由 Core 完成。
- 协调器不持久化、不访问数据库或设置文件、不引入 localStorage。App 退出／真实窗口关闭
  的收尾没有新增生命周期机制；本轮跨页面证据不代替原生关闭窗口验收。
- 保留其他调用方既有 CAS，例如项目包筛选；本轮不改包 resolver、来源策略或 Plan/Apply。
  无新方法、权限、capability、Schema、State migration、依赖或 Rust 生产修改。

## 验证

证据目录：target/m7-settings-autosave-20261030/。

- 前端 check/build 通过；保留既有大 chunk 提示。Node strip-types 协调测试 **6 个场景通过**：
  连续修改／toggle-back、未知提交结果去重、冲突保留无关字段、主题与普通写入顺序、旧快照、
  普通请求及主题任务跨 await 后停止。只读审查发现的停止边界缺口已补检查及回归。
- 即时保存浏览器专项 **6/6**：栏已不存在、窄 PATCH、跨页串行保存、跨页失败重试、未知
  提交不重写、冲突保留主题、普通保存与独立主题确认互不覆盖。
- 主题／动画／三语分段既有组 **36/36**；普通设置相关调用方已改为等待实际保存结果，
  不再点击已移除按钮。主题自己的 Save／Cancel、Portable UI 自身确认仍保留。
- 扩展组首轮 **38/41**：键盘用例还定位旧主题下拉框，已改为真实文字分段 Enter／Space，
  并等待初始页面焦点稳定；最后含该用例和无保存栏截图的复核 **2/2**。
- 剩余两项未闭合：1280px／1440px 的全列表长文本用例，在 Repository packages 注入长
  中英混排内容后，rowHeightChanges 不满足 ≤1px。前面的单行、容器 nowrap、按钮裁切断言
  已通过；后续页面溢出断言尚未执行。未在本轮修改列表行高规则或放宽该断言，不能称
  扩展组或完整 GUI suite 全部通过。待独立定位，详见 regression.log 与对应失败截图。
- cargo xtask check、metadata validation、diff check 通过。没有重复 Rust Workspace／Clippy，
  此次无 Rust 生产修改；历史 Rust 检查点不计成本轮结果。未推送、未运行 Hosted CI。

已检查 settings-without-save-bar.png 的完整窗口：Appearance 之后直接接 System information，
没有全局保存／放弃／已保存卡片；中文与日语截图也保留在证据目录。截图来自隔离 browser
fixture，未冒充真实 Tauri 或屏幕阅读器验收。

唯一工作区 main、HEAD 7d938cf2845cd982c25b12f106a8dac33981c8e7 保持；全部旧 WIP 保留，
本轮未暂存、提交或推送。原 Tauri dev 使用热更新前端，后端无需重建。
Visual Gate 2 仍 PENDING，未进入后续阶段。
