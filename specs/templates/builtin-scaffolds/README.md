# Built-in Unity project data

`v3-projects.zip` contains the tracked Unity project data from ALCOMD3 v3 commit `4aa98ae4f18d42c10137278997180dbede991e88`, under `vrc-get-gui/project-templates/`:

- `avatars-2019.4.31f1`
- `avatars-2022.3.22f1`
- `worlds-2019.4.31f1`
- `worlds-2022.3.22f1`

The ZIP contains 100 original Git blobs under `Assets/`, `Packages/`, and `ProjectSettings/`. SHA-256 is `744f395c10e33383c7fb532fac0ca05ebda84a40c3d97e9a3634926838869bbc`. The MIT license from that directory accompanies the data here, in exported templates, and in the projects created from them.

The v4 archive builder adds template metadata and normalizes the product-name placeholder. The 2019 archives retain the v3 SDK range and add the v4 SDK/Base `3.4.2` locks. Core supplies the actual project name and a unique product GUID when it creates the project.

ALCOMD generated the two projects in `unity6-projects.zip` from the licensed 2022 scaffolds above using Unity 6000.0.67f1 (78a1c2bbeb6a). The archive contains Assets, serialized ProjectSettings and ALCOMD-authored package manifests. UPM package IDs and versions were checked against projects created by VRChat Creator Companion 2.5.0-beta.3. SDK and Base are resolved during project creation. The scaffold license accompanies each exported template. Archive SHA-256 is `7395033e128b692b55cdfca469ca45f2ffdc3c1f290f5e9831042ee35b8cc14e`.
