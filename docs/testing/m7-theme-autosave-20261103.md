# M7 主题统一自动保存

所有者要求移除主题底部的取消/保存特例，沿用设置页自动保存。M7/H7人工状态保持，本记录不替代原生人工验收。

## 最终行为

- 颜色模式、有效HEX/拾色器、九种配色方案与普通偏好共用SettingsAutosave.change，按实际修改字段提交。连续编辑采用同一串行CAS和意图合并，不覆盖未修改字段；保存中仍可编辑。
- 非法或未完成HEX不提交，失焦还原当前有效颜色。主题直接显示队列中的当前意图；过期初始读取或前一写入的响应不覆盖后续编辑。
- 移除独立Save/Cancel、主题草稿会话、主题离开确认、专用失败/重试/成功通知及footer样式和词条。切页后继续保存，失败使用通用设置提醒和显式Retry；重试先重读，未知已提交不重复写。
- 卡片标题的恢复默认仍先确认，仅恢复mode/system、sourceColor/null、scheme/vibrant；已默认时禁用，成功后焦点返回标题。其他卡片和Unity Hub/安装列表例外保持。
- 不改RPC、Config9、State18、权限、默认值或依赖。只经原Core设置用例保存，GUI不写真实设置文件或数据库。

## 验证

GUI check、Material/三语词条边界、metadata与diff检查通过。保存队列40场景通过，原主题显式保存场景改为同一自动队列的串行、停止连接和其他字段保留断言。

首轮集中浏览器57/1；唯一失败为测试将红、绿、白、黑四次有效改色误计为五次，实际四次写入正确，非法HEX未写入。修正计数后，HEX、标签、800/1280桌面常规控件与日语621px主题补验6/0。原失败和补验分别保留于target/theme-autosave-browser及-r2，不拼接为一次全套通过。

修正后整组集中复跑**58/0**（1 worker、0 retries），输出target/theme-autosave-browser-r3，覆盖九方案、浅/深/系统主题、HEX/拾色器、连续编辑合并、失败/冲突/丢响应、同队列其他字段、导航和过期读取、恢复默认及三语布局。已查看该轮深色主题截图，确认底部操作移除、三行控件与标题恢复入口保持。

本轮没有Rust业务、协议、Schema或依赖改动，不重复完整Rust/远端门禁，不提交或推送；浏览器使用隔离夹具，不对真实设置执行恢复或测试改色。

## 桌面更新

GUI release no-bundle及其前端生产构建通过，Rust release编译2分38秒。仅停止已核验旧GUI，Core **151912**持续运行；沿用原profile打开当前GUI **167872**，SHA-256为`EEB35F42965F8792652D18446A4C58BAB5CC6A9286F86C64313B6528F9C94570`，窗口可响应。设置、项目、Editor、Operation、Backup和Status六组公开状态前后一致，138项GUI/测试/UI输入摘要不变。

更新前程序、前后读取及verification.json保留于target/theme-autosave-desktop；当前运行身份同步至target/m7-storage-locations/candidate-runtime.json。没有对真实设置执行恢复、改色或其他测试写入；原生交互观感由所有者查看。
