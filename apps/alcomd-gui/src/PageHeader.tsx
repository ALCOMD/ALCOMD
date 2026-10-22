import type { ReactNode, Ref } from "react";

import { PageTitle } from "./PageTitle";

export function PageHeader({ title, titleId, titleRef, leading, subtitle, children, className = "" }: {
    title: ReactNode;
    titleId?: string;
    titleRef?: Ref<HTMLHeadingElement>;
    leading?: ReactNode;
    subtitle?: ReactNode;
    children?: ReactNode;
    className?: string;
}) {
    const heading = <PageTitle id={titleId} ref={titleRef}>{title}</PageTitle>;
    return (
        <header className={`page-header ${className}`.trim()}>
            {leading}
            {subtitle === undefined ? heading : (
                <div className="page-header-heading">
                    {heading}
                    <div className="page-header-subtitle">{subtitle}</div>
                </div>
            )}
            {children}
        </header>
    );
}
