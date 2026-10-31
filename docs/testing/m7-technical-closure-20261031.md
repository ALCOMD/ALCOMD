# M7 技术收尾证据（2026-10-31）

本记录保存跨平台测试修复与依赖安全维护证据；当前阶段唯一入口仍是 `../status.md`。
H2-A / Visual Gate 2 已签收，H2-B 等待同一技术 SHA 三平台 required jobs、CodeQL 与安全问题收口。
基线为 main `736f55a9eea504442fa8d95e6c26cfc5559a40b7`，不是回退目标。

## 实施范围与当前依赖

| 依赖 | 基线 | 当前精确版本 | 实际边界 |
| --- | --- | --- | --- |
| Wasmtime | 48.0.0 | 48.0.8 | Host-only；async/component-model/cranelift/runtime/std，defaults off；ABI/WIT 不变 |
| rustls | 0.23.43 | 0.23.46 | 既有 reqwest 传递依赖；aws-lc-rs/std/tls12，不增加直接 TLS 库 |
| rustls-webpki | 0.103.14 | 0.103.16 | Linux 系统根证书 verifier 链；不改信任根或证书验证 |
| ZIP | 8.6.0 | 9.1.3 | 用户明确批准 major/API 适配；defaults off，仅 deflate-flate2-zlib-rs |

审批与维护成本直接保存在 `../exec-plans/M6-runtime-dependency-evaluation.md` 和
`../exec-plans/M7-dependency-evaluation.md`，既有 M3/M4 规则及 metadata 版本门禁同步，未放宽校验。
没有新增 ALCOMD 直接生产依赖、unsafe、原生 API 或公共合同，不清空数据和恢复证据。

## 安全来源与计数

官方 RustSec DB 为 `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`（2026-10-30）。基线
`cargo audit` 是 35 项版本匹配；兼容补丁后的中间扫描为 9 项；ZIP 9.1.3 锁定后的实际扫描为
**6 项版本匹配**，均为下述当前桌面配置不启用的 quinn-proto（5）/webpki-root-certs（1）。
另保留 6 unmaintained、2 unsound informational 和 1 已观察 yanked。没有写 ignore 列表，也不
把版本匹配数当成可利用漏洞数或声称 audit 全绿。首次 yanked 在线查询有 517 次 timeout；该
历史局限保持，后续无 stderr 的扫描不用于推定首次查询全覆盖。

ZIP 官方仓库的 18 条公告另外逐项审阅，不把 Dependabot 旧 11 条输入当固定全集。
原始 JSON、官方 release/advisory、目标 feature 图和 Cargo audit 原输出在
`target/m7-technical-closure-20261031/`，本报告保留关键判断和官方链接供提交审阅。

## Hub 同步启动失败与回收

原 macOS CI `41191370985` 的测试把有执行位的纯文本当作必然 spawn 失败。固定 Rust 1.97.1
的 Unix 路径可调用 `posix_spawnp`，Apple libc 在 ENOEXEC 时会经 shell 接受该文本；进程随后
退出与同步启动失败是不同事件。[Rust 源码](https://github.com/rust-lang/rust/blob/1.97.1/library/std/src/sys/process/unix/unix.rs)、
[Apple libc 源码](https://github.com/apple-oss-distributions/Libc/blob/71bbe350ab79eef58113991d817ccc6165061a64/sys/posix_spawn.c)。

生产启动逻辑保持，仅按获批边界澄清注释。Unix 失败夹具改为指向 fixture 内已断言不存在解释器的 shebang，先严格
证明直接 spawn 返回 NotFound，再验证 OpenFailed 与容量回收；Windows 坏 image 保留。新增
系统原生 utility 非零退出仍为接受成功且回收的测试，原 7 项全部保留，Windows 定向 8/8。
既有 application 调用取消模型 1/1；worker 忽略发送失败后继续等待自己创建的 Child，没有
kill 或查询用户 Hub。本轮未运行真实 Hub，也未操作用户 Unity/Hub。

项目所有者已明确批准 No shell 定义为应用不主动构造、调用或通过 shell 执行命令，不拼接
或解释 shell 参数；继续使用 `std::process::Command` 并接受 OS 的 ENOEXEC/解释器回退。
合同原条款直接同步此定义，不声称 OS 绝不会启动 shell。绝对路径、本地所有者、零额外参数、
有界回收和未知结果不自动重试继续保持。spawn 成功仅表示 OS 接受，不能描述为 Hub 已正常
运行、子进程退出成功或窗口就绪。生产启动逻辑不变，不需要原生启动 API。

## 静态调用链证据索引

W1：Cargo.toml；Host main.rs 的静态 bindgen、Component::from_file、StoreLimits、fuel/epoch、
Engine 配置。不可信 Component 可包含内部组合；线性内存/table/instance 限额不代替组件 handles。
W2：固定 Wasmtime Config 的 GC/exception/CM-async 编译 gate；当前无 WASI crate 或并发 guest API。
T：VpmReader 与 PackageCache 的既有 reqwest client，无危险证书开关、自定义 session Debug 或 Brotli/ECH。
Z：archive.rs 与 extension package.rs 在 ZipArchive::new 后才做 entry preflight/签名，文件大小上限
不能排除小 EOCD 输入。路径来自经检验的 CDH，使用自有 create_new 提取，不交给第二套 reader。

## Wasmtime 基线逐公告与处置

| 公告 | 基线/最小48线修复 | 静态判定与条件 | 代码 |
|---|---|---|---|
| [RUSTSEC-2026-0315](https://rustsec.org/advisories/RUSTSEC-2026-0315.html) / CVE-2026-115390, GHSA-m63x-6p34-q65x | 48.0.0 / >=48.0.3, <49.0.0 | **needs_review**：Cranelift+fuel 确实开启；call_ref/try_table 的具体可接受性须按当前 no-GC validator 复核。epoch 保持墙钟中断，不替代 fuel 合同。 | W1,W2 |
| [RUSTSEC-2026-0316](https://rustsec.org/advisories/RUSTSEC-2026-0316.html) / CVE-2026-115389, GHSA-jqpg-j7w6-42pr | 48.0.0 / >=48.0.3, <49.0.0 | **not_actionable**：仅动态 component::Val lifting；当前全部边界经静态 bindgen，未用动态 Val。 | W1 |
| [RUSTSEC-2026-0325](https://rustsec.org/advisories/RUSTSEC-2026-0325.html) / CVE-2026-115383, GHSA-cfhf-m2cr-62wj | 48.0.0 / >=48.0.4, <49.0.0 | **not_actionable**：需要 exceptions/GC；当前 gc feature 关闭，Config::features 将 GC_TYPES/EXCEPTIONS 设为 false。 | W2 |
| [RUSTSEC-2026-0326](https://rustsec.org/advisories/RUSTSEC-2026-0326.html) / CVE-2026-115379, GHSA-hw8m-q44c-ggrf | 48.0.0 / >=48.0.4, <49.0.0 | **not_actionable**：需要 GC+exceptions；当前编译 feature gate 明确关闭。 | W2 |
| [RUSTSEC-2026-0327](https://rustsec.org/advisories/RUSTSEC-2026-0327.html) / CVE-2026-115384, GHSA-32h6-97mm-8q3c | 48.0.0 / >=48.0.4, <49.0.0 | **not_actionable**：需要 guest component-model-async callback；该 feature 未启用且 concurrency_support(false)，不是 Rust async 的同义词。 | W1,W2 |
| [RUSTSEC-2026-0353](https://rustsec.org/advisories/RUSTSEC-2026-0353.html) / GHSA-m6w2-jwq2-cv2c | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要同步 Func::call 调 async-lifted export；当前静态 async binding，且 guest CM async 关闭。 | W1,W2 |
| [RUSTSEC-2026-0354](https://rustsec.org/advisories/RUSTSEC-2026-0354.html) / GHSA-3r89-v5mq-j5xf | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：漏洞位于并发 guest task future 取消；当前 CM async/event-loop 并发 feature 关闭。 | W1,W2 |
| [RUSTSEC-2026-0355](https://rustsec.org/advisories/RUSTSEC-2026-0355.html) / GHSA-63h4-6cx9-pjfj | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 StreamReader<T/U> 往返；宿主没有 stream/future WIT 或相关创建/类型擦除 API。 | W1 |
| [RUSTSEC-2026-0356](https://rustsec.org/advisories/RUSTSEC-2026-0356.html) / GHSA-7v24-42x9-pxf7 | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 CM stream/future realloc/read 调度；当前 CM async feature 关闭。 | W1,W2 |
| [RUSTSEC-2026-0357](https://rustsec.org/advisories/RUSTSEC-2026-0357.html) / GHSA-93g3-36p8-x2gc | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 WASIp3 或 DirectSource/DirectDestination::remaining；二者均未接入。 | W1,W2 |
| [RUSTSEC-2026-0358](https://rustsec.org/advisories/RUSTSEC-2026-0358.html) / GHSA-5hhx-4wrr-3j9g | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 async-lowered import/thread.yield；guest CM async 不进入当前配置。 | W1,W2 |
| [RUSTSEC-2026-0359](https://rustsec.org/advisories/RUSTSEC-2026-0359.html) / GHSA-wjc8-633q-pwxr | 48.0.0 / >=48.0.6, <49.0.0 | **needs_review**：恶意递归 type chain 的 drop 路径可能在 Component::from_file 失败/卸载时触发；未证明 no-GC 配置能完全排除，不能仅凭沙箱关闭。 | W1,W2 |
| [RUSTSEC-2026-0361](https://rustsec.org/advisories/RUSTSEC-2026-0361.html) / GHSA-hmwq-g22x-qjw5 | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 CM async 调度中已排队的调用在 trap 后继续；当前不编译 guest 并发调度器。 | W1,W2 |
| [RUSTSEC-2026-0362](https://rustsec.org/advisories/RUSTSEC-2026-0362.html) / GHSA-vcg5-q8m5-8379 | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 host 异步进入后用同步 API 重入；host functions 仅发 typed daemon call，无同步 Wasm 重入。 | W1 |
| [RUSTSEC-2026-0363](https://rustsec.org/advisories/RUSTSEC-2026-0363.html) / GHSA-q554-3cjc-rx7p | 48.0.0 / >=48.0.6, <49.0.0 | **confirmed**：component resource.new 的 host handle table 不受当前 StoreLimits 约束；不需要 WASI、线性内存、host imports。instances/memories/tables 限额不能替代 store-wide component handle cap。 | W1,W2 |
| [RUSTSEC-2026-0364](https://rustsec.org/advisories/RUSTSEC-2026-0364.html) / GHSA-p79w-pv2r-29qp | 48.0.0 / >=48.0.6, <49.0.0 | **not_actionable**：需要 WebAssembly exceptions；当前 no-gc 编译 gate 禁用 exceptions。 | W2 |
| [RUSTSEC-2026-0378](https://rustsec.org/advisories/RUSTSEC-2026-0378.html) / CVE-2026-115892, GHSA-9pvw-hwfg-92h5 | 48.0.0 / >=48.0.7, <49.0.0 | **not_actionable**：需要 host-created stream/future 经类型擦除往返；当前 WIT/host 无这些 API。 | W1,W2 |
| [RUSTSEC-2026-0379](https://rustsec.org/advisories/RUSTSEC-2026-0379.html) / CVE-2026-115893, GHSA-mghf-wjfp-p5xm | 48.0.0 / >=48.0.7, <49.0.0 | **confirmed**：支持的 macOS arm64 上 Cranelift 直接编译不可信组件；大型 br_table 的条件成立，组件大小上限不排除该输入。Windows/Linux x64 不受此架构特定漏洞影响。 | W1 |
| [RUSTSEC-2026-0422](https://rustsec.org/advisories/RUSTSEC-2026-0422.html) / GHSA-3982-jqvv-277m | 48.0.0 / >=48.0.8, <49.0.0 | **not_actionable**：仅 core TypedFunc<I31/NoneRef>；当前只用 Component Model，官方明确仅组件 embedding 不受影响。 | W1,W2 |
| [RUSTSEC-2026-0423](https://rustsec.org/advisories/RUSTSEC-2026-0423.html) / GHSA-qw58-7h65-crq2 | 48.0.0 / >=48.0.8, <49.0.0 | **not_actionable**：仅 host-created StructType 跨两个 Engine；当前一个 Engine，没有 StructType/supertype embedder API。 | W1,W2 |
| [RUSTSEC-2026-0424](https://rustsec.org/advisories/RUSTSEC-2026-0424.html) / GHSA-6fr9-xmw9-5352 | 48.0.0 / >=48.0.8, <49.0.0 | **confirmed**：组件间借用本组件自有 resource 时，host borrow tracking 不计入 handle cap；只需组件内部组合，不依赖 WASI。当前 StoreLimits 不限制这类 host allocation。 | W1,W2 |

Wasmtime官方新公告与RustSec已逐项对照；48.0.8 release包含0422/0423/0424。GHSA-897f-g5gm-fmxm（RUSTSEC0380）仅v43以前，48已不受影响；GHSA-54p9-vhhq-8f7q（RUSTSEC0360）仅49.0.0-49.0.2，48不受影响。所有wasmtime-wasi/wasmtime-wasi-http条目因对应crate和WASI导入在当前图中缺失而排除；不是基于“有沙箱”排除。


上述 21 条版本匹配均由当前 `48.0.8` 消除；表内 confirmed、not_actionable 和 needs_review 是基线调用链判定。证据不足项没有伪写为不适用，统一升级补丁。修复验证使用既有 Host lifecycle、typed WIT、crash/quarantine、memory/fuel/epoch 和 malformed Component 回归；本轮完整集成结果见验证节。

## TLS 基线逐公告与处置

| 公告 | 基线/修复 | 静态判定 |
|---|---|---|
| [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html) / GHSA-2mjx-qc3c-rqvc | 0.23.43 / >=0.23.45 | **confirmed**：远程仓库/包下载实际经 reqwest→hyper-rustls→rustls，TLS1.3 保持启用；恶意 peer 可触及错误 encryption-level 的握手解析。仅证实 framing 合同受影响，未声称可绕过认证。 |
| [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html) / GHSA-c8m5-c952-6cm4 | 0.23.43 / >=0.23.46 | **not_actionable**：没有自定义 ClientSessionStore/SessionValue Debug 日志，使用 reqwest 内建 cache；官方确认普通连接与内建 cache 诊断仍脱敏。 |
| [RUSTSEC-2026-0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html) / GHSA-h2v8-394m-p7vv | 0.23.43 / >=0.23.46 | **not_actionable**：rustls brotli feature 未激活；没有自定义 certificate decompressor，攻击 decoder 未进入当前生产图。 |
| [RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html) / GHSA-w44r-93j7-qv28 | 0.23.43 / >=0.23.46 | **not_actionable**：当前无 prefer-post-quantum，默认 AWS-LC groups 首项是 X25519、混合组排末；公告所述原本首选混合的连接被降级到 classical 的前提不成立。hint 的旧写入行为仍存在，维护补丁一并修复。 |
| [RUSTSEC-2026-0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html) / GHSA-vp88-798v-h67f | 仅0.23.44/45受影响；0.23.46修复 | 当前0.23.43不在适用范围，且没有EchMode::Enable或握手前写入配置；不选择中间0.23.45作为最终维护候选。 |
| [RUSTSEC-2024-0399](https://rustsec.org/advisories/RUSTSEC-2024-0399.html) | 修复0.23.19 | 当前0.23.43已修复；不是TLS server Acceptor入口。 |
| [RUSTSEC-2024-0336](https://rustsec.org/advisories/RUSTSEC-2024-0336.html) / GHSA-6g7w-8wpp-frhj | 修复0.23.5 | 当前版本已修复；实际tokio-rustls async链不调用blocking complete_io。 |


当前使用 `rustls 0.23.46`、`rustls-webpki 0.103.16`，包含上表及 0382 的维护修复。TLS 仍由 reqwest/platform verifier 校验证书，无关闭证书验证或扩大 feature。现有 HTTP、redirect、hash 回归不能证明上游 TLS handshake 漏洞的 exploit；没有新增 TLS 协议测试框架，真实握手覆盖按验证节如实记录。

## ZIP 官方公告逐项与处置

| 公告 | 基线8.6.0 / 官方patched元数据 | 静态判定 |
|---|---|---|
| [GHSA-gmfj-vffv-32pw](https://github.com/zip-rs/zip2/security/advisories/GHSA-gmfj-vffv-32pw) | 8.6.0 / 9.1.0 | **not_actionable**：仅 ZipStreamReader 严格 footer/offset 验证；当前不使用流式 reader。 |
| [GHSA-5qh5-c9fp-66jm](https://github.com/zip-rs/zip2/security/advisories/GHSA-5qh5-c9fp-66jm) | 8.6.0 / 9.0.0 | **not_actionable**：仅 Windows convenience extraction 会对攻击者 symlink target 做 fs::metadata；当前自有提取先拒绝 symlink，不调用该逻辑。 |
| [GHSA-wjr6-mhvj-cjvm](https://github.com/zip-rs/zip2/security/advisories/GHSA-wjr6-mhvj-cjvm) | 8.6.0 / 9.1.0 | **not_actionable**：ZipArchive::new 的 EOCD fallback 确实存在，但当前检查/读取/提取都用相同中央目录视图；没有交给第二 reader 的审批链，未建立公告所述跨 parser 完整性后果。保留格式歧义事实。 |
| [GHSA-m665-v7pm-7grj](https://github.com/zip-rs/zip2/security/advisories/GHSA-m665-v7pm-7grj) | 8.6.0 / 9.0.5 | **not_actionable**：已核对库的 CDH metadata→by_index→name 与 LFH→payload offset：输出路径从 CDH 已校验路径生成，没有 LFH-name extractor/ZipStreamReader 二次解释。 |
| [GHSA-x2cq-c87r-5mfx](https://github.com/zip-rs/zip2/security/advisories/GHSA-x2cq-c87r-5mfx) | 8.6.0 / 9.0.4 | **confirmed**：zip8.6.0 在 ZipArchive::new 中全文件 EOCD 候选重试，发生在应用 entry/解压配额之前；小于现有文件大小上限的输入仍可触发。 |
| [GHSA-9fw6-6jvx-cj63](https://github.com/zip-rs/zip2/security/advisories/GHSA-9fw6-6jvx-cj63) | 8.6.0 / 9.0.3 | **not_actionable**：legacy-zip 未启用；旧 Shrink/Reduce/Implode decoder 不编译。 |
| [GHSA-8p9w-4prr-vvqh](https://github.com/zip-rs/zip2/security/advisories/GHSA-8p9w-4prr-vvqh) | 8.6.0 / 9.0.3 | **not_actionable**：当前不调用 has_overlapping_files；自身路径碰撞验证不使用此函数。 |
| [GHSA-x2x8-gq67-9mq6](https://github.com/zip-rs/zip2/security/advisories/GHSA-x2x8-gq67-9mq6) | 8.6.0 / 9.0.2 | **not_actionable**：该回归从9.0.0引入；当前8.6.0不在该引入范围，且当前无 convenience extract。 |
| [GHSA-2vxq-ffpp-66cq](https://github.com/zip-rs/zip2/security/advisories/GHSA-2vxq-ffpp-66cq) | 8.6.0 / 9.0.4 | **confirmed**：官方明确确认8.6.0；当前相同两个无界重试循环。上游约1MB恶意输入可通过应用1GiB/64MiB archive上限，且成本在 new 返回前发生。未在本机执行PoC。 |
| [GHSA-c9rm-qm35-rhqf](https://github.com/zip-rs/zip2/security/advisories/GHSA-c9rm-qm35-rhqf) | 8.6.0 / 9.0.1 | **not_actionable**：aes-crypto关闭，无decrypt/password调用，AES HMAC reader不可达。 |
| [GHSA-9hj8-73fh-9963](https://github.com/zip-rs/zip2/security/advisories/GHSA-9hj8-73fh-9963) | 8.6.0 / 9.0.0 | **not_actionable**：legacy-zip关闭；不能用应用在by_index之后的大小检查作为唯一理由，真正排除依据是decoder不编译。 |
| [GHSA-hjqx-j4rw-g7xv](https://github.com/zip-rs/zip2/security/advisories/GHSA-hjqx-j4rw-g7xv) | 8.6.0 / 9.0.0 | **not_actionable**：危险 lexical symlink target validation 在库 extract；当前自有提取拒绝symlink、create_new写入。 |
| [GHSA-8hxf-qr8p-62x4](https://github.com/zip-rs/zip2/security/advisories/GHSA-8hxf-qr8p-62x4) | 8.6.0 / 9.0.0 | **not_actionable**：aes-crypto关闭，无密码；by_index在构建crypto reader前拒绝encrypted entry。 |
| [GHSA-q7v9-x93c-xqv3](https://github.com/zip-rs/zip2/security/advisories/GHSA-q7v9-x93c-xqv3) | 8.6.0 / 9.0.0 | **not_actionable**：convenience extract的symlink target分配不调用；自有preflight拒绝symlink。 |
| [GHSA-4684-h2q9-2vcp](https://github.com/zip-rs/zip2/security/advisories/GHSA-4684-h2q9-2vcp) | 8.6.0 / 9.0.1 | **not_actionable**：不调用库extract/stream extract，不把archive原始unix_mode写成最终权限。 |
| [GHSA-4v9h-p7m9-rh26](https://github.com/zip-rs/zip2/security/advisories/GHSA-4v9h-p7m9-rh26) | 8.6.0 / 9.0.0 | **not_actionable**：库创建外部目标symlink路径不调用；自有reader拒绝link/special file。 |
| [GHSA-vv5m-jxxh-4627](https://github.com/zip-rs/zip2/security/advisories/GHSA-vv5m-jxxh-4627) | 8.6.0 / 9.0.0 | **not_actionable**：库SUID/SGID恢复不调用；自有提取不恢复不受限archive权限。 |
| [GHSA-94vh-gphv-8pm8](https://github.com/zip-rs/zip2/security/advisories/GHSA-94vh-gphv-8pm8) | 8.6.0 / 2.3.0 | **not_actionable**：原始问题已在2.3.0修复；7.2引入的回归独立列为GHSA-hjqx...；当前两者的extract前提均不成立。 |

RustSec当前ZIP匹配为 [RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html) / GHSA-vv5m-jxxh-4627（SUID/SGID）、[RUSTSEC-2026-0368](https://rustsec.org/advisories/RUSTSEC-2026-0368.html) / GHSA-4v9h-p7m9-rh26（symlink target）、[RUSTSEC-2026-0369](https://rustsec.org/advisories/RUSTSEC-2026-0369.html) / GHSA-4684-h2q9-2vcp（symlink permissions）；对应五类信息均见ZIP逐项表和官方JSON。其余更新后的上游GHSA尚未全进入本次DB/Dependabot集合。两项EOCD CPU公告可能覆盖相近根因，保留各自记录，不将重复样貌当作删除输入理由。


当前精确 `zip 9.1.3` 覆盖上述官方 patched 范围，包括 RustSec 尚未收录的 EOCD CPU 公告。修复保留自有路径/类型/链接/碰撞/配额和签名检查，不调用 convenience extract。每项归档类问题的回归共同复用 archive、package、template、backup、extension 安全/事务测试，并增加小型确定性 EOCD work/seek 与名称解码歧义回归；实际结果见验证节。

## 其他版本匹配、informational 与维护状态

| 包 / 公告 | 基线 / 修复 | 可达性或证据不足 |
|---|---|---|
| quinn-proto / [RUSTSEC-2026-0338](https://rustsec.org/advisories/RUSTSEC-2026-0338.html) | 0.11.16 / >=0.11.18 | not_actionable：reqwest HTTP3未启用，当前三桌面没有QUIC入口；Windows cargo tree -i为空。源特定触发条件仍保留在audit JSON，不因版本匹配直接升级未使用协议。 |
| quinn-proto / [RUSTSEC-2026-0339](https://rustsec.org/advisories/RUSTSEC-2026-0339.html) | 0.11.16 / >=0.11.17 | not_actionable：reqwest HTTP3未启用，当前三桌面没有QUIC入口；Windows cargo tree -i为空。源特定触发条件仍保留在audit JSON，不因版本匹配直接升级未使用协议。 |
| quinn-proto / [RUSTSEC-2026-0340](https://rustsec.org/advisories/RUSTSEC-2026-0340.html) | 0.11.16 / >=0.11.18 | not_actionable：reqwest HTTP3未启用，当前三桌面没有QUIC入口；Windows cargo tree -i为空。源特定触发条件仍保留在audit JSON，不因版本匹配直接升级未使用协议。 |
| quinn-proto / [RUSTSEC-2026-0341](https://rustsec.org/advisories/RUSTSEC-2026-0341.html) | 0.11.16 / >=0.11.17 | not_actionable：reqwest HTTP3未启用，当前三桌面没有QUIC入口；Windows cargo tree -i为空。源特定触发条件仍保留在audit JSON，不因版本匹配直接升级未使用协议。 |
| quinn-proto / [RUSTSEC-2026-0342](https://rustsec.org/advisories/RUSTSEC-2026-0342.html) | 0.11.16 / >=0.11.17 | not_actionable：reqwest HTTP3未启用，当前三桌面没有QUIC入口；Windows cargo tree -i为空。源特定触发条件仍保留在audit JSON，不因版本匹配直接升级未使用协议。 |
| rustls-webpki / [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) | 0.103.14 / >=0.103.16, <0.104.0-alpha.1; >=0.104.0-alpha.8 | needs_review：Linux rustls-platform-verifier/others.rs:61-129加载系统roots并构建WebPkiServerVerifier，:133-144验证chain；若系统锚允许emSign路径即受影响。未读取或假定目标机的实际根证书集合；Windows/macOS走系统chain，不将风险误泛化。当前已升级0.103.16。 |
| webpki-root-certs / [RUSTSEC-2026-0383](https://rustsec.org/advisories/RUSTSEC-2026-0383.html) | 1.0.9 / >=1.0.10 | not_actionable（三桌面）：仅rustls-platform-verifier wasm32 target导入，Cargo.toml:119；extension Wasm guest不是host编译target。 |
| tokio / [RUSTSEC-2026-0386](https://rustsec.org/advisories/RUSTSEC-2026-0386.html)（INFO unsound） | 1.53.1 / 1.53.3 | 当前路径not_actionable：全缓存源码按锁版本筛选仅Tokio定义/文档/reexport命中AsyncFd；net TCP(:73-74/164/214)、Unix(:43-44/857)、UDP(:122-123/176)用PollEvented<Mio>持有资源，不调用脆弱constructor。其他cache的dbus-tokio调用不在当前lock。未声称所有未来AsyncFd用法安全。 |
| glib / [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html)（INFO unsound） | 0.18.5 / >=0.20.0（跨GTK线） | 当前路径not_actionable：受影响variant_iter.rs:119-133确实有&p错误；唯一构造入口Variant::array_iter_str(:843/854)，完整cache搜索仅自身cfg(test)调用，无GTK/GIO/Wry/Tauri生产调用；Rust公共API存在不等于当前产品入口可达。不跨0.20擅升。 |
| proc-macro-error / [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html)（unmaintained） | 1.0.4 / 无兼容修复承诺 | 维护状态，不等同于已知可利用漏洞。当前仅记录，不引入替代框架/直接依赖；Tauri/GTK原闭包的后续维护须单独评估。 |
| unic-char-property / [RUSTSEC-2025-0081](https://rustsec.org/advisories/RUSTSEC-2025-0081.html)（unmaintained） | 0.9.0 / 无兼容修复承诺 | 维护状态，不等同于已知可利用漏洞。当前仅记录，不引入替代框架/直接依赖；Tauri/GTK原闭包的后续维护须单独评估。 |
| unic-char-range / [RUSTSEC-2025-0075](https://rustsec.org/advisories/RUSTSEC-2025-0075.html)（unmaintained） | 0.9.0 / 无兼容修复承诺 | 维护状态，不等同于已知可利用漏洞。当前仅记录，不引入替代框架/直接依赖；Tauri/GTK原闭包的后续维护须单独评估。 |
| unic-common / [RUSTSEC-2025-0080](https://rustsec.org/advisories/RUSTSEC-2025-0080.html)（unmaintained） | 0.9.0 / 无兼容修复承诺 | 维护状态，不等同于已知可利用漏洞。当前仅记录，不引入替代框架/直接依赖；Tauri/GTK原闭包的后续维护须单独评估。 |
| unic-ucd-ident / [RUSTSEC-2025-0100](https://rustsec.org/advisories/RUSTSEC-2025-0100.html)（unmaintained） | 0.9.0 / 无兼容修复承诺 | 维护状态，不等同于已知可利用漏洞。当前仅记录，不引入替代框架/直接依赖；Tauri/GTK原闭包的后续维护须单独评估。 |
| unic-ucd-version / [RUSTSEC-2025-0098](https://rustsec.org/advisories/RUSTSEC-2025-0098.html)（unmaintained） | 0.9.0 / 无兼容修复承诺 | 维护状态，不等同于已知可利用漏洞。当前仅记录，不引入替代框架/直接依赖；Tauri/GTK原闭包的后续维护须单独评估。 |
| chacha20（yanked元数据，非RustSec漏洞公告） | 0.10.1 | 已查询到该条yanked；其他517条registry查询timeout，完整yank状态未覆盖。没有由此推定密码算法可利用，也不盲目重解全部lock。 |


完整 workspace 图补核：default-members 不包含 GUI，因此实际使用 `cargo tree --workspace`。
Linux glib 通过 GTK/Tauri 进入 GUI，排除依据是脆弱 array_iter_str API 无生产调用。proc-macro-error
是 Linux GTK 编译宏；五个 unic 经 URLPattern/tauri-utils 同时进入 build 与 runtime，其告警只声明
维护状态，不能笼统描述为 build-only。chacha20 0.10.1 的锁父链为 rand 0.10.2→quinn-proto；当前
Windows/Linux 完整生产图没有 active 路径，未因此把 yanked 当作密码算法漏洞或大规模重解锁图。

## 锁文件逐项变化

37 个现有包更新、1 个新 target 包；其余锁包保持，没有删除包。除 ZIP/nix 外依赖边集合不变。

| 包 | 基线 → 当前 | 必要性 |
| --- | --- | --- |
| `cranelift-assembler-x64` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-assembler-x64-meta` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-bforest` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-bitset` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-codegen` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-codegen-meta` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-codegen-shared` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-control` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-entity` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-frontend` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-isle` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-native` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `cranelift-srcgen` | 0.135.0 → 0.135.8 | Wasmtime 48.0.8 的配套 0.135.8 编译器组件；原 feature/边集合不变 |
| `nix` | 无 → 0.31.3 | 仅 macOS ZIP 的 fs feature；复用既有 bitflags/cfg-if/cfg_aliases/libc |
| `pulley-interpreter` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `pulley-macros` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `rustls` | 0.23.43 → 0.23.46 | 现有 TLS 1.3 framing/维护安全补丁 |
| `rustls-webpki` | 0.103.14 → 0.103.16 | Linux verifier 的 RUSTSEC-2026-0382 安全补丁 |
| `typed-path` | 0.12.3 → 0.13.0 | ZIP 9.1.3 的 ^0.13 reader/path 依赖 |
| `wasm-encoder` | 0.254.0 → 0.254.2 | Wasmtime 新 wit-bindgen 的 ^0.254.1 工具闭包，解析为 0.254.2；非独立 tooling 升级 |
| `wasm-metadata` | 0.254.0 → 0.254.2 | Wasmtime 新 wit-bindgen 的 ^0.254.1 工具闭包，解析为 0.254.2；非独立 tooling 升级 |
| `wasmparser` | 0.254.0 → 0.254.2 | Wasmtime 新 wit-bindgen 的 ^0.254.1 工具闭包，解析为 0.254.2；非独立 tooling 升级 |
| `wasmprinter` | 0.254.0 → 0.254.2 | Wasmtime 新 wit-bindgen 的 ^0.254.1 工具闭包，解析为 0.254.2；非独立 tooling 升级 |
| `wasmtime` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-environ` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-component-macro` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-component-util` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-core` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-cranelift` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-fiber` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-jit-debug` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-jit-icache-coherence` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-unwinder` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-versioned-export-macros` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wasmtime-internal-wit-bindgen` | 48.0.0 → 48.0.8 | 同一 Wasmtime 48.0.8 发布闭包；原 feature/边集合不变 |
| `wit-component` | 0.254.0 → 0.254.2 | Wasmtime 新 wit-bindgen 的 ^0.254.1 工具闭包，解析为 0.254.2；非独立 tooling 升级 |
| `wit-parser` | 0.254.0 → 0.254.2 | Wasmtime 新 wit-bindgen 的 ^0.254.1 工具闭包，解析为 0.254.2；非独立 tooling 升级 |
| `zip` | 8.6.0 → 9.1.3 | 批准的 EOCD 等安全维护；新增 getrandom、macOS nix、Windows windows-sys 依赖边 |

ZIP 复用已存在的 getrandom 0.4.3 与 windows-sys 0.61.2，没有顺带升级它们。nix 的默认 feature
关闭，仅 fs；其 build 使用既有 cfg_aliases，不能描述成零 native 成本。ZIP 请求的 Windows
Foundation/Security/Security_Authorization/Storage_FileSystem feature 在原 platform 依赖中已启用，
增加 ZIP 调用边不等同于整个产品首次引入这些系统权限 API。

## 浏览器跨平台失败收敛

基线 CI 41191370985 的 Windows/Ubuntu browser 失败独立调查。Windows 两项已用有界探针
复现：SplitButton 在实际 Segoe UI 回退/分数字体宽度下，原采样点位于正确圆角的抗锯齿边缘；
测试改采明确的顶部背景像素，仍精确要求两角与 gap 相同、主体不同并保留 8px 几何。故意
把外半径设为 9999px 的错误反例仍使两角断言失败，没有增加颜色容差。

另一项测试在 menu 的 opening 与 opened 之间发送 Escape，焦点仍在 trigger，未触发 menu
关闭。仅测试改为预注册、等待公开 opened/closed，再检查原禁用和焦点条件；生产组件保持。
原五项 Windows 本地基线 5/5，修正后两项定向 2/2，独立记录，不拼为完整通过。

`73f465b` 的 Windows 完整 required job 成功，Chromium 为 **247/0（5.8 分钟）**；原 candidate
harness heading 缺失在这一轮未出现，不据此补造旧失败根因。Ubuntu 完整构建和 Rust 检查通过，
Chromium 为 **245/2（6.0 分钟）**，仍是 header 56→64 与 resize 后 Project 列保持 180。
本轮有界诊断确认实际字体为 DejaVu Sans：固定 204px 的 Unity Select 成为 64px，撑高 header；
1440px 窗口的列表 viewport 为 1120px，但六列固有总宽为 1143.984375px，没有可分配剩余空间。

本机诊断仅在测试页加载现有 Poppler 随附 DejaVuSans，精确复现同一两行 label、64px header、
1143.984375px 表格及六列宽度；没有安装或新增产品字体。现仅将既有 Unity Select 宽度由固定
204 改为 max-content、保留 204 下界与不可压缩布局；该字体下变为 212.375×40，header 恢复
56，其他共享 Select 不变。表格测试按实际 overflow 加原 160px 增量建立“存在剩余空间”的
前提，继续精确要求四个信息列与 Actions 不变、Project 增宽；DejaVu 对照在 1624px 窗口下
Project 为 340.015625，其余五列完全相同。新增仅对该 Select 使用公开 monospace 字体 token
的回归，同时检查单行高度、完整选中内容、无横溢及不遮挡 Open Unity。相关三项定向 **3/3**。
证据为 gui-ubuntu-font/probe-widthonly.json、ci-73f-ubuntu-job.log 和 gui-ubuntu-targeted.log；
完整候选套件单独执行，不将定向结果拼接为通过。系统 TCP、浏览器参数、30 秒门槛及零重试保持。

平台与精确输入、probe/反例及命令保存在 `target/m7-technical-closure-20261031/gui-summary.md`
和各 gui-* 子目录。本机为 Windows 11 Pro 10.0.26300、Node 24.22.1、Playwright 1.62.1、
Chromium 151.0.7922.34，不代称 Server 2025 或 Ubuntu 通过。

## 验证与未覆盖范围

ZIP API 适配初轮：VPM 库 111/111、扩展库 10/10、备份 RPC 8/8；2 个 ignored 是由通过
矩阵实际调用的 subprocess fixture。fmt 与四个适配文件的 diff 检查通过。小输入旧版对照
使用 16/32/64 个 EOCD；64 项输入为 7312 字节，共享目录下旧 8.6.0 读取 297472 字节/12679
次 seek，新 9.1.3 为 9993/139；重叠目录旧为 188608/6631，新为 5511/115。两版均拒绝
畸形归档，只有新版满足两种线性上界，不用时间或大输入作为判据。

有效归档使用 Stored/Deflate × DOS/Unix creator × 两版 writer × 两版 reader × 5 个成员，
共 **80 份完整成员断言通过**，包括双向跨版本和同版本对照。ASCII、嵌套 UTF-8/emoji、文本、
二进制与空文件的名称、受限相对路径、内容、CRC、大小、固定时间、原始属性、普通文件类型及
无加密/链接均核对。两版 Stored 输出均为 666 字节、Deflate 均为 679 字节；不据此承诺任意
第三方归档的编码字节完全相同。证据为 zip-compatibility-profile.log 与原有独立 harness。

同一 DOS 归档的原始高 16 位均为 `100644`，旧 reader 优先返回它，新 reader 按 DOS 属性投影为
`100664`；Unix creator 两版均为 `100644`。产品不从该投影恢复源文件 Unix 权限，不能把 reader
差异误写成 writer 格式变化。初始相等假设失败及同字节控制输出保持在原日志。

该变化也会将 DOS 归档原始声明的 symlink/FIFO 推导为普通文件，令旧的仅 derived-mode 校验
错误接受。VPM preflight 与已签名扩展检查的新增回归在修复前均实跑失败，合法 DOS 普通文件
对照通过。最终两处同时验证原始类型声明与推导类型，继续使用原错误、允许类型和提取规则；
修复后同一命令 **3/3 通过**，没有放宽路径、配额、链接、签名或 create_new 防御。未观察到
自有提取器创建链接；这里修正的是既有拒绝声明合同。red/green 分别保存在
zip-dos-type-before.log 与 zip-dos-type-after.log。

完整门禁首轮 `scripts/check.ps1` 的 fmt/Clippy 已通过，Workspace test 尚未运行：链接时
C 盘仅约 545 MB，MSVC 明确报 LNK1180 insufficient disk、PDB LNK1318 LIMIT/FILE_SYSTEM
和 LNK1108。核实没有 cargo/rustc、绝对目录正确且非 reparse 后，仅删除可重建的
`target/debug/incremental` 编译缓存，保留 debug/release 程序、全部日志/截图/数据与
`target/worktree-recovery`；空间恢复到约 51 GB。仅重置日志明确列出的失败链接中间文件，
记录在 build-cache-cleanup.json/failed-link-artifacts-reset.json。恢复轮完整门禁 exit 0，Workspace
为 488/0/8；DOS 类型修复后的最终完整 `scripts/check.ps1` 另为 **491 passed / 0 failed /
8 既有 helper ignored**，exit 0，541 个验证输入前后未变。该最终轮包括 xtask、fmt、Workspace
Clippy、Workspace tests、GUI Rust check、Discord 独立合同、根 npm check/build 与 metadata。

首轮完整 browser 为 **243 passed / 4 failed（14.7 分钟）**。候选页原始诊断明确捕获
browser-harness 文档请求 ERR_ADDRESS_IN_USE，主题 zh-CN 几何用例明确报 ENOSPC 写入失败；
ja-JP html.lang 和 Projects grid 取消后的焦点断言分别依据原 trace 调查。grid 有界事件探针
复现 Escape 发生于 opening 与 opened 之间、焦点仍在 trigger；等待 opened 后 Escape 正常
closed 并返回原控件。ja-JP 原失败 trace 的选项在点击时仍受展开动画裁剪、位置变化；新探针
三次均成功，不称作新鲜复现。两个测试沿用既有公开 opened/closed 等待，全部原断言保持，
相关定向 **4/4 通过（34 秒）**。不将全部失败归因环境，也不把定向通过拼成全套通过。
原 trace、截图、日志、输入和 exit 均保留。

第二轮完整 browser 为 **246 passed / 1 failed（14.0 分钟）**，150 个输入前后不变，时段无
新增 Tcpip 4227。唯一失败是四宽度/双主题的 grid 布局测试耗尽 30 秒总预算：原 trace 中
BeforeHooks→AfterHooks 为 30011.997 毫秒，164 个已完成协议动作没有 error，56 个请求全部
200/304，没有失败请求或 page error；正文随后仍完成最后截图、滚动与 Backups 导航断言。
四宽度分别约 8.978/6.699/6.877/6.794 秒，页面 fixture 另约 1.831 秒。失败后一分多钟的
CPU 100% 只作同时负载观测，不据此宣称因果。构建结束后，同一原用例的独立定向为 1/1
（23.3 秒），不拼接为完整通过。全部四宽度、两主题、12 张截图、精确断言及原 30 秒门槛
保持；后续完整回归按 CI 顺序在全部构建后单独执行。

第三轮独立完整 browser 为 **237 passed / 10 failed（10.8 分钟）**，exit 1，150 个输入
前后不变。全部 10 份 trace 各自的 HAR 均有 ERR_NO_BUFFER_SPACE / status -1：3 项文档请求
失败，最终为 about:blank；7 项 HTML 为 200，但必需的静态 JS 失败，after-goto 的 root
children=0。10 项均没有实际 input 事件，不能把它们描述为业务断言通过。

| 未到达的业务用例 | 失败资源 | 原始证据 |
| --- | --- | --- |
| package confirmation | harness document | goto 失败，about:blank |
| shared button spacing/label width | PagedItems JS | HTML 200，空 root |
| system/no-preference animation | harness document | goto 失败，about:blank |
| stale package versions/pre-Plan | UtilityWorkspace JS | HTML 200，空 root |
| changed package source | PagedItems JS | HTML 200，空 root |
| zh-CN last-modified terminology | PagedItems JS | HTML 200，空 root |
| grid capabilities/missing Editor | harness document | goto 失败，about:blank |
| en-US theme segment geometry | PagedItems JS | HTML 200，空 root |
| Editor refresh after Hub save | PagedItems JS | HTML 200，空 root |
| ordinary save preserves Hub draft | styles JS | HTML 200，空 root |

三个 JS 都是实际构建入口的直接静态 import，不是仅凭可选 preload 失败推断。逐项 URL、
UTC、快照和最后 API 保存在 gui-verified-offline 的 report.md/classification.json 与原 trace。
本轮相关时段没有 Tcpip 4227/4231；结束后 TCP 聚合为 1145 项、动态范围 16384，仍不能从
结束时快照排除运行中的瞬态问题，也未证明端口耗尽。独立 Node loopback 对照最多 48 次、
并发 6、零重试，实际 48/48；它不证明 Chromium 的失败已经消失。没有发现本任务残留的
验证浏览器或服务器可供回收，没有修改系统、代理或浏览器配置，也没有关闭用户进程。

[Chromium Windows 错误映射](https://chromium.googlesource.com/chromium/src/+/be46c60078d52078152906df781aea0728978390/net/base/net_errors_win.cc)
将 WSAENOBUFS 映射为此错误；具体 Windows 根因尚未确定，不能将错误名直接等同于端口耗尽。
[Microsoft 端口排障说明](https://learn.microsoft.com/en-us/troubleshoot/windows-client/networking/tcp-ip-port-exhaustion-troubleshooting)
要求结合事件与连接等证据。当前本机完整 GUI 门禁仍失败，其记录保留，不拼接为通过。

| 首个技术提交 `73f465b` 的本地验证 | 已取得结果 | 日志 |
| --- | --- | --- |
| 完整 Rust/前端门禁 | PASS；Workspace 491/0/8 | local-check-final.log |
| 冻结基线只读核验 | PASS；未重写 source-lock 或旧只读仓库 | baselines-final.log |
| 正式 debug daemon/CLI | PASS | debug-bins-final.log |
| 并发 CLI 自动启动 | PASS；一个权威 daemon，测试后回收 | auto-start-final.log |
| Workspace release bins | PASS | release-bins-final.log |
| Tauri release no-bundle | PASS | tauri-release-final.log |
| 完整 Chromium，最后独立轮次 | FAIL；237/10，10 项均未到达业务 | gui-verified.log |

该提交全部构建后再次核对，三个 lockfile 与最终门禁的 541 个源码/测试输入保持。其 GUI SHA-256 为
`E74AD19BFE00BDA405AF71D40AD2D401ABDCC62CBEF28D760621FDCBC8729685`，其余 daemon/CLI/Host
hash 保存在 production-binary-hashes.json。没有以构建替代真实桌面产品验收。

原样例 Core 与原 Tauri dev 命令已恢复，正式 CLI/RPC 读回的 system status、两个 Projects、
空 Editor 列表、Operation 列表和全部 Settings 与任务前一致，settings revision 155。
原生窗口 accessibility 已确认 Projects 页面和两条样例行加载；这是恢复开发环境的证据，
不是 release GUI 或 H2-B 的真实桌面验收。相关比较和窗口记录保存在 sample-restored-comparison.json、
restored-dev-accessibility.txt 与 restored-dev-window.json。

本地结果不继承基线 Hosted CI/CodeQL；必须另核对本轮实际技术提交 SHA 的全部远端结果。
项目所有者已明确批准仅 `73f465b4cd0ab28b90794fcaac94e21db5be0e72` 普通快进推送到
`origin/main`，保留上述本机失败记录，以该 SHA 三平台完整 required jobs 与 CodeQL 全绿验证
本轮技术收尾。推送后已核实远端与本机 HEAD 一致；没有强推、改历史或发布。
[三平台 CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41204615658) 已全部结束：Windows 成功，
macOS 在 Check 失败，Ubuntu 在上述两项 Chromium 布局断言失败；同 SHA 的
[Push CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41204615683)
和 [Scheduled CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41204687960) 均已成功，四种语言
检查全部通过。没有三平台完整 PASS 结论，H2-B 尚未开始。

macOS 的 Hub 8 项（含确定同步拒绝、非零 child 退出、接收方取消和回收）实际全部通过。
后续候选 RPC 在 `m7_candidates_rpc.rs:148` 因 fixture 的长 TMPDIR 加描述性目录/完整 UUID/
runtime/socket 后缀超过 portable sockaddr_un 103 字节而失败。原 job 日志为
ci-73f-macos-job.log，下载自已完成 job 124547054483；未重跑这个已知会失败的原 SHA。

该 fixture 现沿用 M4/M5/M7 其他 RPC 测试已有的 macOS `/private/tmp` 私有临时根，其他平台
继续使用原 temp_dir，保留完整 UUID、目录层级、Drop 与所有业务断言。最终 socket 为 91 字节，
生产 103 字节检查、owner/NOFOLLOW/0700/0600 和通常运行时路径保持。没有新增临时目录框架、
生产回退或测试跳过。候选 RPC 定向实际 **1/1**；修复后完整 scripts/check.ps1 另为
**491/0/8**、exit 0，541 个输入前后不变，日志为 macos-fixture-targeted.log 和
macos-fixture-check.log。该修改只有测试夹具；macOS 的修复效果仍等待包含它的新 SHA 三平台检查。
本轮再次暂停/恢复原样例，五项正式 RPC 结果全部一致，settings revision 155，证据为
sample-followup-restored-comparison.json。后续 CSS 与测试修复没有再次暂停或修改原 Core 数据。

## 后续技术候选的本地结果

候选 RPC 短临时根修复后的 Workspace Rust 输入已由 macos-fixture-check.log 完整覆盖，
结果 **491 passed / 0 failed / 8 既有 helper ignored**。此后只有 CSS 和 browser spec 两个验证
输入变化；541 项快照核对没有额外 Rust/合同变化。根 npm check/build、Tauri release no-bundle、
metadata 均通过，三个 lockfile 未变。当前 release GUI SHA-256 为
`F049B03D117F8FD964496BC334597679084516DE10EDF164390B01037C71EBC4`。

独立完整 Chromium 为 **246 passed / 2 failed（15.5 分钟）**，exit 1，150 个输入全程未变。
这不是完整 GUI PASS，不能由布局专项 3/3 或前一 SHA 的 Windows 247/0 代替。

| 失败用例 | 原始时间线与实际覆盖 |
| --- | --- |
| Missing source requires affirmative Core evidence | 唯一正式失败为 30 秒 test budget。前两组流程在 16.175/26.508 秒完成；第三组在 29.133 秒通过 Missing source 计数，下一次 Available click 于 29.866 秒开始并跨越预算。trace 中 30 秒以后的动作不能计为按时通过。 |
| 257 targets and selected rows are disabled | `open` 的标题可见断言失败；该调用从 1.406 秒开始，标称 5 秒，ExpectError 记录于 8.020 秒。1.585 秒的录屏主区仍空，7.111 秒的帧已有标题与表格，首次渲染只能界定在两帧之间；不能以迟到截图声称可见性断言通过。257 项批量业务断言均未到达。 |

两份 trace 各自的 21/7 个网络记录全部为 200，没有 pageError 或明确阻塞网络证据；既有
React 开发诊断另外记录，不与 pageError 混淆。当前时段无新增 Tcpip 4227/4231，也不能据此
证明本机系统或调度因素不存在。没有将本轮失败归为 ERR_NO_BUFFER_SPACE，没有改超时、
拆分用例、重试、浏览器 flags、系统配置或包管理实现，也没有无诊断重跑整套。

本轮日志、trace 和输入分别为 gui-ubuntu-final.log、gui-ubuntu-final-results、
gui-ubuntu-final-inputs-before/after.json；目录名表示本轮修复来源，实际运行平台仍为 Windows。
离线时间线另存 gui-ubuntu-offline/README.md。原样例五项正式 RPC 再次完全一致，revision 155，
没有遗留的 harness server，证据为 sample-layout-comparison.json 和 gui-ubuntu-final-tcp-events.json。
项目所有者已另行明确批准精确后续技术 SHA `02b6f0c3e81b8eedb711123ef9de2edc228792ab` 普通
快进推送，保留本轮 246/2 失败，以该 SHA 三平台完整 required jobs 与 CodeQL 全绿验证技术收尾。
实际推送后已核实 `origin/main` 为该 SHA；
[CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41209763063) 已全部结束：Windows 与 macOS 完整
required job 成功，完整 Chromium 分别为 **248/0（6.9 分钟）**和 **248/0（8.7 分钟）**；Ubuntu
完整 Chromium 为 **247/1（6.6 分钟）**，唯一失败是 257 目标用例的 5 秒标题检查。此前两项
Ubuntu 布局失败及 macOS socket 长路径失败已不再出现，但本 SHA 仍不是三平台 PASS。
[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41209762434) 已四语言成功。完整各平台日志与
实际 job 结果保存为 ci-02b6f0c-{windows,macos,ubuntu}-job.log 和 ci-02b6f0c-final.json。H2-B 仍未开始，
不能将前一 SHA 的 Windows/CodeQL 成功继承给本轮。

推送时 GitHub 提示仍有 11 条 open Dependabot 告警。本轮重新通过官方 API 获取，告警编号与
GHSA 集合与任务开始时完全相同，没有新增或移除；zip 五条、Wasmtime 两条、rustls 四条均已在
上表逐项评估，
当前 9.1.3/48.0.8/0.23.46 不在这些版本影响范围。未将 GitHub open 状态谎称为 resolved，未
人工变更告警。证据为 dependabot-open-after-02b6f0c.json 与
dependabot-after-02b6f0c-comparison.json；RustSec 实际剩余
6 项未启用 QUIC/wasm32 匹配与 informational/yanked 仍分别保留，不拿 GitHub 数量替代 audit。

本轮没有真实受影响证书链或上游 TLS handshake PoC 验证；使用官方维护补丁、
既有证书验证路径检查及网络回归验证产品兼容，不以 HTTP mock 代替 TLS 漏洞复现。
前一兼容补丁检查在用户将 ZIP 目标改为 9.1.3 后
主动取消，日志保留，不将部分编译视为完整门禁通过。metadata 同步后已通过。

## 大列表初始化的后续技术候选（2026-11-01）

`02b6f0c` 的唯一远端失败和本机相同标题失败均保留。最新原始 trace 的主区在 t0.861 秒为空，
t4.190 秒已绘制标题和 260 行表；标题断言从 t0.669 秒开始，t5.831 秒报错。迟到 DOM 中的
`H2#packages-heading` 具有原 role/name、没有 hidden/inert/aria-hidden；不能据此倒推原断言按时通过。
Playwright 首次未匹配时生成整页 ARIA 诊断确有额外成本，但同一个 5 秒总预算的可见性等待和
已有 ID 等待诊断仍分别失败，因此没有修改正式标题等待方式，也不把 ARIA 归为唯一根因。

同一有界剖析中，原 260 行初始有 525 个菜单 host、1051 个菜单项，其中 1040 个是未打开的行
More 项，DOM counter 为 220726。按需创建两类行菜单后，该轮初始为 5 个 host、11 个项及
84980 个 DOM 节点；这些是诊断 counter，不是可见行数。仅延迟子项、仅延迟 More host、
同时延迟两类菜单的候选都没有单独闭合标题门槛；仅使用按需菜单的两文件回归为 **50/1（3.3 分钟）**。
仅增加绘制机会的独立候选虽有原用例 1/1，其原菜单剖析仍失败。所有这些
轮次独立保存在 gui-batch-load、gui-row-menus、gui-present-targeted 与 gui-present-only，不能
挑选通过轮次拼成修复结论。

浏览器样式失效时间线进一步记录到 desktop-list-viewport 的 19 次全子树失效，来自既有
OverlayScrollbars 的 `noContent` 直接子项通配选择器；viewport 属性的其他 token 变化同样会
触发保守失效，每轮约 19835 个元素。CSS 读取的耗时落在同一 viewport 的 direction、width、
display 测量。只阻止两个 overflow 变量继承的诊断没有取得收益；正式代码没有修改 vendor、
依赖或 CSS 规则，也未修改浏览器 flags、系统配置、超时或批量数据来规避它。原始 timeline、CPU profile、帧和
API 摘要保存在 gui-batch-load；重算总成本未被宣称消除。

当前组合候选只修改两个已有生产文件：PackageVersionMenu/PackageRowMoreMenu 首次打开时
创建整个 Material Menu，随后持续连接以完成原 closed/焦点回调；ScrollArea 在同步测量前给
内容一次 rendering opportunity，卸载继续取消当前待执行帧并销毁实例。它不保证浏览器已实际
绘制或全部 Material 更新已完成。260 行、原数据、来源与权限、批量上限和 Plan/Apply 保持。
原版本菜单测试将公开 opened/closed 监听放在稳定的 row capture 上，以适配 host 的首次创建；
新增一项 More 首次键盘 Space 选择、重新打开/Escape、返回焦点和单次 intent 回归。

两份包管理测试、滚动条和导航四文件的组合候选回归为 **63 passed / 0 failed（5.2 分钟）**，
单 worker、零重试。原 257 目标用例实际 **12286 ms / 30000 ms**，原 5 秒标题检查和全部业务
断言保留；Missing source 原三组场景为 13848 ms，未拆分或扩大预算。该轮还覆盖滚轮、横向
滚动、thumb 拖动、卸载清理、Material 键盘/焦点与导航。日志及逐项时长为
gui-present-menus.log/.json/.exit。根 npm check/build、Tauri release no-bundle、xtask 与 metadata
全部通过，日志为 row-render-{check,build,tauri,xtask}；release 构建用时 3 分 08 秒。
当前 GUI SHA-256 为 `283F06EEC69D51F55BB27419090EB0D7A80C2F1C8A0E8C7E0BF7758E4A3DD400`。

全部构建结束后，独立完整 Chromium 为 **247 passed / 2 failed（18.4 分钟）**，exit 1；249 项、
单 worker、零重试，150 个输入前后完全一致，GUI hash 未变。原 257 目标实际 12756 ms，
Missing source 三组场景 12452 ms，新增 More 首次键盘回归 4393 ms，均通过；这些不能拼接
为完整 GUI PASS。完整结果为 gui-row-render-final.log/.json/.exit、对应 results 和
gui-row-render-final-inputs-before/after.json。

| 本轮完整失败 | 已确认的覆盖与未覆盖 |
| --- | --- |
| same-route reconnect | HTML 已取得，但入口实际静态 import 的 PagedItems JS GET 为 status -1 / ERR_NO_BUFFER_SPACE，rootChildren=0。初始断线提示的 5 秒断言失败，恢复事件与能力恢复业务均未到达；没有把它归为事件订阅竞态或 rAF 回归。 |
| Projects grid 四宽度/双主题 | 累计 30 秒预算耗尽。已记录 42 个网络请求为 32×200 / 10×304，无失败请求或 pageError；browser trace 在 1440 滚动处结束，不覆盖后续网络。1180、1280 的两主题/滚动/Backups 和 1440 两主题在预算内完成，1440 dark PNG 在 29.833 秒完成。30.022 秒已进入 After Hooks；1440 滚动/Backups、1600 sparse 和 7 卡/padding 的随后成功均为迟到，不计通过；1600 双主题几何、两张 dense 截图及后续滚动/Backups未到达。不能仅凭该 trace 判断环境或新 rAF 的性能因果。 |

两项分别在 gui-row-render-final-offline 的 cd7ae-reconnect-readonly.md 和
cacf1-grid-readonly.md 记录。前者保留完整资源 URL、UTC、空 root、真实入口 import 和文件
hash；后者保存逐动作的预算内/迟到/未达分类、网络与帧，36 条 React ARIA 诊断和 pageError
分开，不称 console 干净。没有扩大 5 秒/30 秒、拆分测试、裁减四宽度或截图、重复全套直到
绿色、改系统/浏览器配置或关闭用户进程。有效事件核对窗口为本地 00:10:51.303673 至
00:29:35.563048，Tcpip 4227/4231 为 0；这不证明系统或瞬态资源问题不存在，也不将错误名
等同于端口耗尽。

原样例五项正式 RPC 再次完全一致，settings revision 155；没有暂停原 Core/dev、写入其数据或
遗留 harness 监听，证据为 sample-row-render-comparison.json。RustSec 官方 HEAD 仍为
`2295466c4e9df26647fd67d8fa1c705ac2fc7f81`；GitHub 当前 11 条 open 告警的编号、GHSA、
影响范围、修复版本、scope、状态与更新时间均与 `02b6f0c` 后的读取相同，当前 pins 的逐项
适用性结论保持。证据为 rustsec-head-row-render.json 与 dependabot-row-render-comparison.json；
没有人工修改告警状态。

541 项输入与完整 Rust 491/0/8 的基线对比，新增差异仅上述两个 GUI 文件及候选 browser spec；
此前 `02b6f0c` 的 CSS 与 Material 测试差异另有原门禁，Rust、锁文件和公共合同没有再变化，
记录为 row-render-input-delta.json。本机完整门禁仍失败；项目所有者在审阅实际结果后另行明确
批准仅 `1932140a815dd12712cd00108d70acebe51a3560` 的普通快进推送，保留 247/2，以该 SHA
三平台完整 required jobs 与 CodeQL 全绿验证技术收尾。推送后已核实本机 HEAD、tracking ref 与
实际 `origin/main` 均为该 SHA，没有强推、改历史或推送标签，见 push-1932140-confirmed.json。
[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41221092450) 已全部结束，四种语言均成功，
记录为 codeql-1932140-final.json；[CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41221092743)
已全部结束：Ubuntu 完整成功、browser **249/0（5.5 分钟）**，macOS 完整成功、browser
**249/0（9.2 分钟）**；Windows browser **248/1（6.9 分钟）**，唯一失败是 system/no-preference
动画用例的 scrollbar transitionDuration 为 0 秒，原断言要求大于 0.01 秒。日志和最终状态分别
保存在 ci-1932140-final.json、ci-1932140-{ubuntu,macos}-job-api.log 和 ci-1932140-windows-job.log。
该 run 的 artifacts API 返回 0 项，无法取得其失败截图和 trace，不把推测写成远端根因。
仍须同一最终技术 SHA 三平台完整 required jobs 成功，才能开始 H2-B。各轮失败与输入身份独立
保留，不将定向结果拼接为全套通过。H2-B 尚未开始。

## 动画样式等待的后续测试候选（2026-11-01）

既有 ScrollArea 的初始化位于后续 rendering opportunity；控件偏好已同步不代表滚动条样式
已经可作为最终结果读取。原测试对样式仅 evaluate 一次，其余控件检查使用自动等待断言。
本地六组合观测分别覆盖 system/full/reduced 与系统 reduce/no-preference；第一次 DOM
出现及随后八帧都读到相符的 0.15 秒或 0.001 毫秒、已连接的 light DOM 元素，网络和 pageError
均无失败。该观测没有复现远端的 0 秒，也不能确定远端唯一原因；原始观测保存在
gui-motion-followup/probe.{mjs,json,log,exit}。

候选仅把该测试的最终样式检查改成现有 expect.poll，在默认断言时限内重新读取当前 locator，
保持正常分支 >0.01 秒、减弱分支 <0.001 秒；另增加既有六个 transition property 的精确校验，
避免样式未加载的零时长让减弱分支误通过。生产代码、样式、依赖、5 秒/30 秒预算与零重试均未改。
动画专项为 **9/0（27.6 秒）**，根 npm check/build、metadata 与 diff check 通过，分别保存为
gui-motion-followup/targeted-corrected、check 和 build 的 log/exit。541 项基线核对再次确认
Rust、公共合同与锁文件未变，完整 Rust 491/0/8 的证据保持；release GUI 仍为上述
`283F06EE…3DD400`，没有因测试变更重建或启动新的 release GUI，详见 rust-and-release-inputs.json。

之后的独立完整 Chromium 为 **248 passed / 1 failed（12.6 分钟）**，exit 1，249 项、单 worker、
零重试；150 个输入前后完全一致。动画九项、257 目标、Missing source 和四宽度/双主题 Projects
网格均通过；这些不能替代完整门禁通过。唯一失败为 Hub direct-typing 用例打开 Settings 时
Select 的 5 秒可见性检查：HTML 返回 200，实际静态 import 的 PagedItems JS GET 为 status -1 /
ERR_ADDRESS_IN_USE，trace 中 React rootChildren=0。输入、Enter 保存、blur 和焦点业务均未到达，
没有将其归为 Hub 保存或焦点回归，也没有仅凭网络错误名判断全局端口耗尽。完整资源 URL、
UTC、空 root、真实入口 import、bundle/trace hash 及未覆盖边界见 full-failure-evidence.json。
本轮没有重跑整套、增加超时/重试、裁减用例、改系统/浏览器配置或修改 Hub 实现。

日志与结果为 gui-motion-followup/full.{log,json,exit}、full-results 和
full-inputs-before/after.json。原样例五项正式 RPC 再次完全一致、settings revision 155，
harness 4173 无监听，见 sample-comparison.json。官方 RustSec HEAD 仍相同，GitHub 11 条 open
告警的编号、GHSA、影响范围、修复版本、scope、状态与更新时间均与 `02b6f0c` 后的读取相同，
没有人工修改告警状态，见 dependabot-comparison.json。

本机完整门禁仍失败；仅此前 `73f465b`、`02b6f0c` 与 `1932140` 有各自精确 SHA 的失败门禁
推送例外，新候选不能自行继承。仍待同一最终技术 SHA 三平台完整 required jobs 与 CodeQL
成功，不开始 H2-B。
