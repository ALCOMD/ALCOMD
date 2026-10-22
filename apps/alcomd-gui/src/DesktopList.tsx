import { keyboardArrowDownIcon, keyboardArrowUpIcon } from "@alcomd/ui/icons";
import type { CSSProperties, ReactNode } from "react";

import { Button, Icon } from "./Material";
import { ScrollArea } from "./ScrollArea";

export type DesktopListSortDirection = "ascending" | "descending";

export interface DesktopListColumn {
    key: string;
    header: ReactNode;
    width?: CSSProperties["width"];
    className?: string;
    /** Text and controls never soft-wrap by default; stacked content keeps its explicit lines. */
    cellFlow?: "truncate" | "wrap";
    onSort?(): void;
    sortDirection?: DesktopListSortDirection;
}

export interface DesktopListRow {
    key: string;
    cells: ReactNode[];
}

export function DesktopList({
    className,
    columns,
    emptyMessage,
    label,
    minWidth,
    rows,
    sizing = "content"
}: {
    className?: string;
    columns: readonly DesktopListColumn[];
    emptyMessage?: ReactNode;
    label: string;
    minWidth?: number;
    rows: readonly DesktopListRow[];
    sizing?: "content" | "fixed";
}) {
    if (columns.length === 0) throw new Error("DesktopList requires at least one column");
    for (const row of rows) {
        if (row.cells.length !== columns.length) throw new Error(`DesktopList row ${row.key} does not match its column count`);
    }
    const style = minWidth === undefined ? undefined : { "--desktop-list-min-width": `${minWidth}px` } as CSSProperties;
    return (
        <ScrollArea className="desktop-list-viewport">
            <table
                aria-label={label}
                className={["desktop-list", className].filter(Boolean).join(" ")}
                data-sizing={sizing}
                style={style}
            >
                <colgroup>{columns.map((column) => <col key={column.key} style={column.width === undefined ? undefined : { width: column.width }} />)}</colgroup>
                <thead><tr>{columns.map((column) => <DesktopListHeader column={column} key={column.key} />)}</tr></thead>
                <tbody>
                    {rows.map((row) => <tr key={row.key}>{row.cells.map((cell, index) => {
                        const column = columns[index];
                        return <td className={column?.className} data-cell-flow={column?.cellFlow ?? "truncate"} key={column?.key}>{cell}</td>;
                    })}</tr>)}
                    {rows.length === 0 && emptyMessage !== undefined ? <tr><td className="desktop-list-empty" colSpan={columns.length}>{emptyMessage}</td></tr> : null}
                </tbody>
            </table>
        </ScrollArea>
    );
}

function DesktopListHeader({ column }: { column: DesktopListColumn }) {
    const sortable = column.onSort !== undefined;
    return (
        <th
            aria-sort={sortable ? (column.sortDirection ?? "none") : undefined}
            className={column.className}
            data-sort-active={column.sortDirection === undefined ? "false" : "true"}
            data-sortable={sortable ? "true" : "false"}
            scope="col"
        >
            {sortable ? (
                <Button className={`desktop-list-sort${column.sortDirection === undefined ? "" : " desktop-list-sort--active"}`} onClick={column.onSort} type="button" variant="text">
                    <span className="desktop-list-sort-layout">
                        <span aria-hidden="true" className="desktop-list-sort-measure"><span className="desktop-list-sort-measure-icon" /><span>{column.header}</span></span>
                        <span className="desktop-list-sort-content">
                            <span className="desktop-list-sort-icon" data-active={column.sortDirection === undefined ? "false" : "true"}><Icon asset={column.sortDirection === "ascending" ? keyboardArrowUpIcon : keyboardArrowDownIcon} size={20} /></span>
                            <span className="desktop-list-sort-label">{column.header}</span>
                        </span>
                    </span>
                </Button>
            ) : column.header}
        </th>
    );
}
