# M7 设置卡片恢复默认

状态：实现、定向回归及匹配桌面GUI更新完成。M7/H7人工状态保持。

## 最终范围

所有者要求标题同行提供恢复默认，并在每次恢复前确认。最新明确Unity Hub路径和Unity安装列表作为例外，保留原按钮；其余六卡片实现：项目、备份、全局Unity启动参数、外观、窗口、主题。
恢复只提交当前卡片字段，路径清除覆盖回系统默认，项目自动添加true，备份fast/保留包true/Library false，启动参数null，locale/motion=system，rememberState=false，主题system/null/vibrant。
旧主题footer与启动参数编辑弹窗内重复的恢复入口移到卡片标题，不保留两套入口。

安装列表重置的空登记/重新发现提案已按最终例外取消，未新增RPC、权限、Schema或Core逻辑。

## 保存和交互

- 使用现有settings.get/update与串行队列；打开确认时固定目标字段基线，确认后先重读，目标字段外部变化时拒绝，其他卡片字段不覆盖。
- 未知写入结果保持弹窗，重试先核对实际值；已经提交则不重复写。相同默认值不提交无差异PATCH。
- 取消不提交，也不清除未保存目录/主题草稿。指针打开及键盘Shift+Tab到恢复按钮都抑制目录失焦提交；主题仅成功后重建其编辑会话、清除本地预览。
- 当前卡片值全为默认且没有目录草稿或主题预览时，标题恢复按钮禁用。显式路径覆盖和启动参数空数组均不等同于nullable默认值。
- 确认Dialog保持挂载完成关闭与焦点恢复；取消后回到对应标题按钮，确认成功后按钮禁用则回到卡片标题。保存中禁止重复提交/关闭。

## 实际验证

- GUI check、根前端build、词条/Material边界及metadata通过。
- 保存队列40场景通过：原29保留，新增六卡片窄PATCH、排队CAS、丢响应去重及字段冲突/无关字段保留。
- 浏览器首轮26/9：6个新/调整用例scope命中Shadow内部dialog，无法读slotted正文或按钮；2个旧三语用例未按卡片限定多个同名恢复按钮；1个旧主题操作点击被成功通知遮挡，改为先正常关闭通知，保留原取消/保存断言。
- 第二轮33/3：一项新外观用例将已有en-US误算作一次写入；另两项暴露关闭时提前卸载Dialog中断焦点恢复与关闭完成前继续编辑的边界。修正默认基线、保持Dialog完成关闭，并以真实焦点返回作为后续输入的前置条件。独立只读审查另发现键盘从dirty路径跳到标题按钮会触发旧blur保存，已统一识别跨Shadow的恢复触发元素并补键盘取消回归。
- 修正后新增卡片组独立5/0。最终集中**53/0**（1 worker、0 retries），覆盖恢复/取消/丢响应、目录草稿、主题保存/预览/取消、全局和项目启动参数、三语分组及原目录操作；不声称完整浏览器套件通过。输出依次为target/settings-reset-browser、-r2、-r3、-r4，保留各轮。
- 已查看-r4中文桌面尺寸截图，确认标题和按钮同行。未重复Rust完整门禁：本轮没有Rust、协议、Schema或依赖改动，605/0仍是此前六档压缩的历史结果。

### 默认值禁用增量

按所有者后续要求增加六卡片默认值判断，并保留目录草稿、主题预览的恢复入口；恢复成功后按钮禁用，焦点返回卡片标题。启动参数null禁用，显式空数组允许恢复。

GUI check、metadata与diff检查通过。首次沙箱运行因其独立用户缓存没有Chromium而无法启动浏览器，已中断，未执行产品交互；改在真实用户环境使用已安装浏览器，最终集中**55/0**（1 worker、0 retries），输出target/settings-reset-disabled-browser-r2。保留既有恢复/保存/目录/三语分组断言，新增默认目录草稿取消/Escape/确认且零写入、默认主题预览恢复，以及恢复后禁用和焦点断言。已查看该轮中文截图，确认项目与备份的恢复按钮呈禁用状态；本增量未改保存队列或Rust，不重复其既有门禁。

## 桌面更新

沿用本轮查看效果授权，只构建并重开GUI，Core保持运行，不执行真实设置恢复。初次只读核对引用旧reader被方法白名单拒绝，发生在停止GUI之前；保留before-reader-rejected，修正reader路径后才进行更新。源码快照、原GUI及前后六组公开读取保存于target/settings-card-reset-desktop。

GUI release no-bundle通过（2分55秒），保留原profile重开GUI **149848**，窗口可响应，SHA-256为`56F35B58E491AAAAAD4498DA587D287F5E4B0B345D21979EA37577EDE2C197DD`。Core **151912**持续运行；Project、Editor登记、Operation、Settings、Status及Backup六组公开读取与更新前完全一致，未对真实设置执行恢复。138项GUI/测试/UI样式输入在构建期间hash一致。最新运行身份同步至target/m7-storage-locations/candidate-runtime.json。不提交、推送或新增原生人工验收。

默认值禁用增量的GUI release no-bundle再次通过（Rust release编译3分06秒），沿用原profile打开当前GUI **173596**，SHA-256为`314FE831AD213A81C63C706451ECA0FE4854E819CD9B483E1990C1BCDEDD8E81`，窗口可响应。Core **151912**未关闭，六组公开状态全部一致，138项GUI/测试/UI输入摘要不变；保留更新前二进制及前后读取于target/settings-card-reset-disabled-desktop，当前运行身份仍同步至target/m7-storage-locations/candidate-runtime.json。不对真实设置执行恢复，不提交或推送，不冒称原生交互人工验收。
