export type SupportedLocale = "en-US" | "zh-CN" | "ja-JP";
export type MessageValues = Readonly<Record<string, string | number>>;
export type Translate = (message: string, values?: MessageValues) => string;
export type TranslationTable = Readonly<Record<string, Readonly<Record<Exclude<SupportedLocale, "en-US">, string>>>>;
