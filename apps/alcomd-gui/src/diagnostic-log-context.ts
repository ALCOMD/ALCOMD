import { createContext } from "react";
import type { DiagnosticLogFeed } from "./diagnostic-log";

export const DiagnosticLogContext = createContext<DiagnosticLogFeed | undefined>(undefined);
