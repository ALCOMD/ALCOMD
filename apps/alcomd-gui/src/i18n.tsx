import { createContext, Fragment, useContext, useMemo, type ReactNode } from "react";

import type { MessageValues, SupportedLocale, Translate, TranslationTable } from "./i18n-types";
import { shellMessages } from "./locales/shell";
import { pageMessages } from "./locales/pages";
import { actionMessages } from "./locales/actions";
import { formMessages } from "./locales/forms";

export type { SupportedLocale, Translate } from "./i18n-types";

// English messages are the source keys. External package/project/extension text never enters this catalog.
export const messages: TranslationTable = { ...shellMessages, ...pageMessages, ...actionMessages, ...formMessages };
const LocaleContext = createContext<SupportedLocale>("en-US");

export function preferredLocale(value: string): SupportedLocale {
    const language = value.replaceAll("_", "-").toLowerCase().split("-")[0];
    if (language === "zh") return "zh-CN";
    if (language === "ja") return "ja-JP";
    return "en-US";
}

export function createTranslator(locale: SupportedLocale): Translate {
    return (message: string, values?: MessageValues) => {
        const pattern = locale === "en-US" ? message : messages[message]?.[locale] ?? message;
        return pattern.replace(/\{([a-zA-Z][a-zA-Z0-9]*)\}/g, (placeholder, name: string) => String(values?.[name] ?? placeholder));
    };
}

export function LocaleProvider({ children, locale }: { children: ReactNode; locale: SupportedLocale }) {
    return <LocaleContext.Provider value={locale}>{children}</LocaleContext.Provider>;
}

export function useTranslation(): { locale: SupportedLocale; t: Translate } {
    const locale = useContext(LocaleContext);
    const t = useMemo(() => createTranslator(locale), [locale]);
    return { locale, t };
}

/** Interpolate React nodes without interpreting translated text as HTML. */
export function Translated({ message, values }: { message: string; values: Readonly<Record<string, ReactNode>> }) {
    const { t } = useTranslation();
    return <>{t(message).split(/(\{[a-zA-Z][a-zA-Z0-9]*\})/g).map((part, index) =>
        <Fragment key={index}>{/^\{[a-zA-Z][a-zA-Z0-9]*\}$/.test(part) ? values[part.slice(1, -1)] ?? part : part}</Fragment>
    )}</>;
}
