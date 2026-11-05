import { Component, type ReactNode } from "react";
import { Button } from "./Material";
import { useTranslation } from "./i18n";

export class RouteErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
    override state = { failed: false };
    static getDerivedStateFromError() { return { failed: true }; }
    override render() { return this.state.failed ? <RouteFailure retry={() => this.setState({ failed: false })} /> : this.props.children; }
}

function RouteFailure({ retry }: { retry(): void }) {
    const { t } = useTranslation();
    return <div><p role="alert">{t("This view could not be displayed. Open another page or try again.")}</p><Button type="button" onClick={retry}>{t("Retry")}</Button></div>;
}
