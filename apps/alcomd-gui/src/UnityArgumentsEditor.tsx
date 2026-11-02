import { Button, TextField } from "./Material";
import { useTranslation } from "./i18n";

/** Keep each argv entry intact, including empty values, spaces, quotes and backslashes. */
export function UnityArgumentsEditor({ value, onChange, disabled = false, readOnly = false, id }: {
    value: string[]; onChange(value: string[]): void; disabled?: boolean; readOnly?: boolean; id: string;
}) {
    const { t } = useTranslation();
    return <div className="unity-arguments-editor">
        <p className="settings-supporting-text">{t("Enter each option and its value separately. Paths with spaces stay in one field; no extra quotes are needed.")}</p>
        {value.length === 0 ? <p className="settings-supporting-text">{t("No additional arguments.")}</p> : value.map((argument, index) => <div className="unity-argument-row" key={index}>
            <TextField disabled={disabled} readOnly={readOnly} id={`${id}-${index}`} label={t("Argument {number}", { number: index + 1 })}
                type="textarea" rows={1} value={argument} onInput={(next) => onChange(value.map((item, position) => position === index ? next : item))} />
            {readOnly ? null : <Button disabled={disabled} aria-label={t("Remove argument {number}", { number: index + 1 })} onClick={() => onChange(value.filter((_, position) => position !== index))} type="button" variant="text">{t("Remove")}</Button>}
        </div>)}
        {readOnly ? null : <Button disabled={disabled} onClick={() => onChange([...value, ""])} type="button" variant="text">{t("Add argument")}</Button>}
    </div>;
}
