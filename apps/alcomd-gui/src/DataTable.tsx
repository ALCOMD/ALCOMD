import { keyboardArrowDownIcon, keyboardArrowUpIcon } from "@alcomd/ui/icons";
import type { CSSProperties, ReactNode } from "react";

import { Button, Icon } from "./Material";

type SortDirection = "ascending" | "descending";

export function MaterialDataTable({
    children,
    className,
    label,
    minWidth = 720
}: {
    children: ReactNode;
    className?: string;
    label: string;
    minWidth?: number;
}) {
    const style = { "--material-data-table-min-width": `${minWidth}px` } as CSSProperties;
    return (
        <div className="material-data-table-scroll">
            <table aria-label={label} className={["material-data-table", className].filter(Boolean).join(" ")} style={style}>
                {children}
            </table>
        </div>
    );
}

export function DataTableHeader({
    children,
    className,
    onSort,
    sortDirection
}: {
    children: ReactNode;
    className?: string;
    onSort?(): void;
    sortDirection?: SortDirection;
}) {
    const sortable = onSort !== undefined;
    return (
        <th
            aria-sort={sortable ? (sortDirection ?? "none") : undefined}
            className={className}
            data-sort-active={sortDirection === undefined ? "false" : "true"}
            data-sortable={sortable ? "true" : "false"}
            scope="col"
        >
            {sortable ? (
                <Button
                    className={`material-data-table-sort${sortDirection === undefined ? "" : " material-data-table-sort--active"}`}
                    onClick={onSort}
                    type="button"
                    variant="text"
                >
                    <span className="material-data-table-sort-layout">
                        <span aria-hidden="true" className="material-data-table-sort-measure">
                            <span className="material-data-table-sort-measure-icon" />
                            <span>{children}</span>
                        </span>
                        <span className="material-data-table-sort-content">
                            <span className="material-data-table-sort-icon" data-active={sortDirection === undefined ? "false" : "true"}>
                                <Icon asset={sortDirection === "ascending" ? keyboardArrowUpIcon : keyboardArrowDownIcon} size={20} />
                            </span>
                            <span className="material-data-table-sort-label">{children}</span>
                        </span>
                    </span>
                </Button>
            ) : children}
        </th>
    );
}
