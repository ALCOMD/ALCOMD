import metadata from "../package.json" with { type: "json" };

// validate-metadata.py keeps this build version aligned with alcomd.product.toml and Tauri.
// It describes this GUI, independently of the connected Core's version or availability.
export const guiVersion = metadata.version;
