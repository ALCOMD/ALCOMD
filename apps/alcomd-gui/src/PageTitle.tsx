import type { ReactNode, Ref } from "react";

export function PageTitle({ children, id = "route-title", ref }: {
    children: ReactNode;
    id?: string;
    ref?: Ref<HTMLHeadingElement>;
}) {
    return <h1 className="page-title" id={id} ref={ref} tabIndex={-1}>{children}</h1>;
}
