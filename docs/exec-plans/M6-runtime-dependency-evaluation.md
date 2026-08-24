# M6 Runtime dependency evaluation (Stop A candidate)

状态：评估已获项目所有者批准并按精确配置落盘；本文件记录选择依据与最终 active graph 约束。

评估日期：2026-08-24。来源：Wasmtime 官方 [release policy](https://docs.wasmtime.dev/stability-release.html)、
[Wasmtime repository](https://github.com/bytecodealliance/wasmtime)、crates.io published metadata 与 docs.rs crate
manifest。隔离 probe 不属于 Workspace，评估结束后删除。

## 推荐候选

```toml
wasmtime = {
    version = "=48.0.0",
    default-features = false,
    features = ["async", "component-model", "cranelift", "runtime", "std"],
}

ed25519-dalek = {
    version = "=3.0.0",
    default-features = false,
}
```

placement：`wasmtime` 只作为 `apps/alcomd-extension-host` 的直接 production dependency；`ed25519-dalek` 只作为
`crates/alcomd-extensions` 的直接 production dependency。其他 crate 不直接依赖；Wasmtime/Ed25519 类型不泄露到
domain/application/protocol 或公共 RPC。

### Wasmtime 48.0.0

- 发布：2026-08-20；48 是 divisible-by-12 LTS line，官方支持 24 个月并保证 supported line 的 security fix
  backport。ALCOMD 固定 major 48，允许并要求升级兼容 security/critical-correctness patch。
- license：`Apache-2.0 WITH LLVM-exception`；MSRV Rust 1.95.0，Workspace 1.97.1 满足。
- exact direct features 仅 `async, component-model, cranelift, runtime, std`；defaults off。
- `async` 用于 async host use case、wall timeout/cancellation；`component-model` 用于 WIT Component；`cranelift`
  用于安装后本机 compile；`runtime/std` 是执行基础。
- 明确不启用 cache、wat、parallel-compilation/rayon、profiling、pooling-allocator、threads、GC、coredump、
  debug-builtins、all-arch、Winch、WASI p1/p3。
- fuel 与 epoch interruption 是 runtime config，不需要额外 Cargo feature；exact combined policy 已冻结在
  `runtime-limits-v1.json`。

### WASI / WIT tooling

- 第一条 slice 的 exact WASI crate set 是空：不直接依赖 `wasmtime-wasi`，不链接 clocks/random/filesystem/socket/
  cli 等任何 ambient WASI 0.2 interface。若未来批准一个 exact WASI interface，再单独评估
  `wasmtime-wasi = 48.x`、defaults off、只启用 `p2`。
- direct `wit-component`/`wit-parser` dependency 不需要。Wasmtime 48 component macro 的 active locked closure 已包含
  `wit-component 0.254.0` 与 `wit-parser 0.254.0`；production 使用 versioned WIT +
  `wasmtime::component::bindgen!`/Component validation，不另加 0.257.x，避免两套 tooling version。
- guest language 不冻结；只要求输出 exact Component world。ALCOMD 不为第一方提供 privileged SDK。

### Ed25519 3.0.0

- 用途仅为 `.alcomdext` strict detached signature verification；不生成/持久化 signing key，不建立 PKI/CA。
- license `BSD-3-Clause`；MSRV Rust 1.85；pure Rust，crate 声明禁止 unsafe，no build script。
- defaults off、零 features；安装频率低，不启用 `fast` precomputed table、rand、pkcs8、pem、serde、batch、digest。
- package/public-key/signature 使用 fixed lowercase hex，因此不新增 base64/PEM dependency；现有 `sha2 0.11.0`
  计算 content digest/fingerprint。

## Active feature/build evidence

隔离评估与实际 Workspace `cargo tree -e features` 均只显示 Wasmtime direct features
`async/component-model/cranelift/runtime/std`；
`wasmtime-wasi` 与 `rayon` 均不在 graph。`wit-component 0.254.0` 只由
`wasmtime-internal-wit-bindgen -> wasmtime-internal-component-macro -> wasmtime` 引入。Ed25519 没有 active feature。

Wasmtime/Cranelift 内部包含第三方 unsafe、JIT executable memory、platform signal/unwind/runtime code，并运行自身
build scripts/source generation（含 `cc`/Cranelift generator closure）。这不扩大 ALCOMD 三文件 unsafe whitelist，
但属于 production approval 的供应链与平台风险。Windows x86_64、Ubuntu x86_64、macOS arm64 都需真实 hosted
compile/run/resource-limit tests。

## Isolated size/time probe

Windows x86_64、Rust 1.97.1、release profile、cold dependency compile：

| probe | result |
|---|---:|
| candidate cold build | 291.641 s |
| candidate executable | 10,305,536 bytes |
| empty executable | 115,200 bytes |
| measured probe delta | 10,190,336 bytes |

这是只实例化 Engine/Config 并引用 verifier 的 isolated probe，不是最终 `alcomd-extension-host` 发行体积。最终产品
delta 仍必须在生产候选三平台测量；该结果证明 compile-time/size cost 显著，不能启用 Wasmtime defaults。

## Isolated Cargo.lock delta

相对当前根 lock，排除 evaluation root package 后新增 68 个 exact `(name, version)`：

```text
addr2line 0.26.1
allocator-api2 0.2.21
arbitrary 1.4.2
async-trait 0.1.92
cc 1.4.4
cobs 0.3.0
cpp_demangle 0.5.1
cranelift-assembler-x64 0.135.0
cranelift-assembler-x64-meta 0.135.0
cranelift-bforest 0.135.0
cranelift-bitset 0.135.0
cranelift-codegen 0.135.0
cranelift-codegen-meta 0.135.0
cranelift-codegen-shared 0.135.0
cranelift-control 0.135.0
cranelift-entity 0.135.0
cranelift-frontend 0.135.0
cranelift-isle 0.135.0
cranelift-native 0.135.0
cranelift-srcgen 0.135.0
crc32fast 1.5.1
curve25519-dalek 5.0.0
curve25519-dalek-derive 0.1.1
ed25519 3.0.0
ed25519-dalek 3.0.0
either 1.18.0
embedded-io 0.4.0
embedded-io 0.6.1
encoding_rs 0.8.35
fiat-crypto 0.3.0
find-msvc-tools 0.1.11
futures 0.3.34
gimli 0.33.0
hashbrown 0.16.1
id-arena 2.3.0
itertools 0.14.0
leb128fmt 0.1.0
libm 0.2.16
log 0.4.34
mach2 0.6.0
memfd 0.6.5
object 0.39.1
postcard 1.1.3
pulley-interpreter 48.0.0
pulley-macros 48.0.0
regalloc2 0.15.2
rustc-demangle 0.1.28
signature 3.0.0
target-lexicon 0.13.5
termcolor 1.4.1
wasm-encoder 0.254.0
wasm-metadata 0.254.0
wasmparser 0.254.0
wasmprinter 0.254.0
wasmtime 48.0.0
wasmtime-environ 48.0.0
wasmtime-internal-component-macro 48.0.0
wasmtime-internal-component-util 48.0.0
wasmtime-internal-core 48.0.0
wasmtime-internal-cranelift 48.0.0
wasmtime-internal-fiber 48.0.0
wasmtime-internal-jit-debug 48.0.0
wasmtime-internal-jit-icache-coherence 48.0.0
wasmtime-internal-unwinder 48.0.0
wasmtime-internal-versioned-export-macros 48.0.0
wasmtime-internal-wit-bindgen 48.0.0
wit-component 0.254.0
wit-parser 0.254.0
```

部分现有 package/version 被复用，target-only items 可锁定但不得描述成 active runtime component。生产批准后必须
重新生成真实 Workspace lock diff，并拒绝无法由 exact features 解释的额外 package。

## Rejected alternatives

- Wasmtime defaults：引入 cache/wat/rayon/profiling/GC/threads/debug 等未需要能力，拒绝。
- `wasmtime-wasi` p2：第一条 slice 无批准 WASI import，会扩大依赖与 ambient-authority 误用风险，暂拒绝。
- Wasmtime normal monthly line：只有 2 个月支持，不符合固定 LTS major，拒绝。
- native plugin/Wasmer/Wasmi/Wasmedge：偏离已批准 Wasmtime LTS/Component Model 或缺少同等既定维护路径，拒绝。
- direct latest `wit-component`：会与 Wasmtime locked tooling 形成第二版本，拒绝。
- OpenSSL/platform signing/复杂 PKI：跨平台/native/权限范围过大；M6 只需 Ed25519 verify，拒绝。

## Stop A approval outcome

项目所有者已批准上面的 exact `wasmtime 48.0.0` 与 `ed25519-dalek 3.0.0` 配置、预期锁定闭包及第三方
unsafe/native/build-script cost。实际 Workspace 已按上述 placement 落盘；没有直接 `wasmtime-wasi` 或
`wit-component` dependency，没有扩大 ALCOMD unsafe whitelist。最终三平台 active graph 与 runtime/resource-limit
行为仍由 M6 最终提交对应的 Hosted CI 验收。
