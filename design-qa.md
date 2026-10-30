# Theme editor layout and Material component review

Ordinary Settings now save immediately and the page-wide Save/Discard/saved-status bar is removed.
Independent Theme Save/Cancel remains. Current full-window evidence is
target/m7-settings-autosave-20261030/settings-without-save-bar.png. The autosave group passed 6/6,
the theme/motion group 36/36 and the final focused recheck 2/2. Broader list text-height assertions
at 1280/1440px remain unresolved; this UI review is not a complete GUI gate pass.

The supplied image is a reference for expanded information layout only. Pixel or control-style cloning is outside the requested result.

Current evidence: target/m7-theme-md3-20261030/theme-light-window.png and theme-dark-window.png, with matching dialog crops. These are browser fixture captures; Visual Gate 2 remains PENDING.

The two-column layout and nine visible options remain. Controls use the installed Material Web implementations: standard outlined text field, standard outlined segmented buttons, radios and sliders. The temporary tonal/pill overrides, HEX pill styling, decorative HCT strips and dialog background override are removed rather than retained as alternate variants. Existing product-wide 28px card geometry, Noto fallback fonts and dynamic theme are retained as owner choices, not described as MD3 defaults. Low-container panels keep the full default slider track visible.

Review covers typography/readability, semantic surface colors, consistent spacing, all choices visible, component focus and selected states, footer fully inside the real dialog, keyboard single-selection and the accepted explicit Save/Cancel workflow. No image assets or new dependencies are needed.

Shared segmented sizing now reserves each label's selected footprint before selection, including the optional checkmark, using a hidden noninteractive intrinsic sizing layer with matching font tokens. Chinese/Japanese/English remain single-line and the group size stays stable across choices. The visible Material controls retain their own styles. Current evidence: target/m7-segment-sizing-20261030/chinese-system-selected.png, with six theme cases passing and a separate 3/3 navigation/Chinese recheck. No locale-specific or page-specific button widths are introduced.

Saving keeps the existing static Save label and disabled request lock, without transient visible text, alternate-label sizing or a status paragraph. Dialog and footer geometry remain stable during a held save; error content is retained until successful retry closes the dialog. This focused group passed 6/6 in target/m7-theme-save-stability-20261030/, including a regression that reproduced a 32px vertical movement before the fix.

The current text-segment/layout/save-cancel group passed 5/5, covering Light/System/Dark order, selected checkmarks, absence of mode icons and single-selection. The earlier layout/keyboard/settings group passed 4/4. Native screen reader and owner desktop signoff are not claimed. This report replaces the earlier screenshot-cloning assessment.
