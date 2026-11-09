# 固定表头的滚动条与软件包安装图标

2026-11-09，M7局部GUI修正。所有者要求列表轨道不覆盖固定表头，并将软件包安装图标改为圆圈加号，与移除的圆圈叉号对应。此前GUI和Core未提交改动保留，本轮不提交、推送或更新运行Core/GUI。

共享DesktopList观察表头布局高度，将纵向滚动条起点设在表头下方，保持原生表格列对齐、固定表头、单一滚动容器和现有OverlayScrollbars交互。使用布局高度避免CSS缩放后重复乘算偏移；组件卸载清理观察者。水平滚动条及普通页面/菜单滚动条不变。

安装使用官方Material Symbols `add_circle`，与`cancel`同为Rounded、weight 400、grade 0、fill 0、opsz 24。原样取自现有固定上游提交`e083cc60a0828fdd3b404cea0cb8a5b900e9c23e`，新增静态SVG、封闭图标导出及manifest摘要，未新增依赖或运行时网络请求。只替换软件包安装动作，其他下载图标不变。

证据目录：`target/m7-package-scrollbar-icon-20261109/`。全工作区前端检查和生产构建通过，35份静态图标资产边界通过；已有大于500KB bundle提示保留。三个软件包浏览器文件及共享滚动条文件覆盖45项不同用例：首轮44通过，1项预发布选择用例在异步请求发出前读取空数组而失败。保留所有参数、错误呈现和页面断言，仅将请求断言改为等待真实请求，再检查无新增提示；该用例独立连续3轮通过，未自动重试或跳过。原失败、截图及trace保存在`browser.log`和`browser-output/`，复验在`browser-race.log`及`browser-race-output/`，不把首轮计为45项全通过。

Playwright CLI另核对同一构建的真实溢出列表：表头底边及滚动条顶边均为209.796875px；滚动到末尾后表头位置保持，轨道底边不越过容器，最后一行可达。动态表头72px及125%缩放后仍对齐（缩放时布局偏移48px、实际表头60px）。图标读取为cancel/add_circle/add_circle，浅色和深色窄窗口截图均已实际查看：`output/playwright/package-scrollbar-light.png`、`package-scrollbar-dark-narrow.png`。几何与缩放记录为同目录`geometry-final.log`、`geometry-resize-zoom.log`。这些是合成Core客户端浏览器证据，不代替原生GUI验收。

初始CLI在沙箱账户因Chrome crashpad管道退出，转真实Windows用户隔离会话后完成；最初480px高的两行夹具不溢出，未将它计为滚动验收，最终改用三行真实候选夹具产生246px内容/218px视口。开发夹具已有React ARIA/Lit提示及favicon 404保留，没有新增React错误。专用CLI浏览器和预览已关闭。未操作真实项目，不改变100ms、包操作合同或失败呈现，Rust/完整workspace/Hosted CI未重跑，M7/H7保持。
