# Public repository visibility fixtures

Frozen on 2026-11-10 from publisher-owned package listings, for parsing and Store/candidate regression only. Each file retains one unmodified declaration per sampled package; tests do not fetch its archive. No reference implementation code is included.

- `nadena.json`: https://vpm.nadena.dev/vpm.json — four packages, including Modular Avatar - Resonite support without author. Raw snapshot SHA-256 `eb64bd7709c99f9f86b4baa2615b75393f7529006f2b05fa1857268370b652e6`.
- `lilxyzw.json`: https://lilxyzw.github.io/vpm-repos/vpm.json — eleven packages, all without author. Raw snapshot SHA-256 `11cb5ebbfeba1271027230e8d9c85e99a37f795ab904e5bcf489458f970deea7`.
- `vrm-converter.json`: https://esperecyan.github.io/VRMConverterForVRChat/registry.json — UniGLTF 0.128.1, with a null GUID in legacyFiles. Raw snapshot SHA-256 `ab535b4804d93d2c1d09c06093c0f93f65e09833b4d80830ff750c80f19afadd`.

Consumption semantics were checked against Unity 2022.3 Package manifest documentation (optional object/string author, recommended displayName/description) and VRChat VPM Packages documentation (path-only legacy declarations). VPM publishing guidance lists additional required presentation fields; consumers still need to handle existing listings without hiding otherwise usable packages.
