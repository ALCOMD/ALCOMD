# ALCOMD Material Symbols assets

The official GUI vendors only the Google Material Symbols it actually renders. Runtime builds never load icons from Google Fonts, GitHub, a CDN, or an npm icon package.

The frozen source and axes are recorded in `manifest.toml`. Every SVG is copied unchanged from the pinned `google/material-design-icons` commit. The shared `@alcomd/ui` icon API selects the exact optical-size asset; CSS must not crop, scale, or otherwise reshape individual symbols.

Add a symbol only when production UI uses it. Use the same upstream commit and add the exact Rounded, weight 400, grade 0, fill 0 asset for the required 20 px or 24 px optical size. A fill-1 asset is allowed only for an explicit selected or toggled semantic state.
