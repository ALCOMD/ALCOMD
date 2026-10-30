import { expect, test } from "@playwright/test";
import { createTranslator, messages, preferredLocale } from "../../src/i18n";

test("system language matching supports the current three languages and regional tags", () => {
    for (const tag of ["zh-CN", "zh_Hans_CN", "zh-Hans", "zh"]) expect(preferredLocale(tag)).toBe("zh-CN");
    for (const tag of ["ja", "ja-JP", "JA_jp"]) expect(preferredLocale(tag)).toBe("ja-JP");
    for (const tag of ["en-GB", "en", "de-DE", "", "unknown"]) expect(preferredLocale(tag)).toBe("en-US");
});

test("translation interpolation preserves external values and stable codes verbatim", () => {
    const text = "<script>literal & {code}</script> C:\\Projects\\日本語";
    const key = "Unavailable because {capability} was not negotiated.";
    expect(createTranslator("en-US")(key, { capability: text })).toBe(`Unavailable because ${text} was not negotiated.`);
    for (const locale of ["zh-CN", "ja-JP"] as const) {
        const translated = createTranslator(locale)(key, { capability: text });
        expect(translated).toContain(text);
        expect(translated).not.toContain("{capability}");
        expect(createTranslator(locale)("unknown_external_package_name")).toBe("unknown_external_package_name");
    }
});

test("every catalog message supplies both translations with exactly the source placeholders", () => {
    const names = (message: string) => [...message.matchAll(/\{([a-zA-Z][a-zA-Z0-9]*)\}/g)].map((match) => match[1]).sort();
    for (const [key, translations] of Object.entries(messages)) {
        for (const locale of ["zh-CN", "ja-JP"] as const) {
            expect(translations[locale].trim(), `${locale}: ${key}`).not.toBe("");
            expect(names(translations[locale]), `${locale}: ${key}`).toEqual(names(key));
        }
    }
});
