import {
  api,
  type ExtractAnalysis,
  type ExtractionRun,
  type ExtractComponent,
  type ExtractPlan,
  type ExtractResult,
  type ExtractScan,
} from "../api";
import { notify } from "./ui.svelte";

export type ExtractStrategy = "copy" | "move" | "history";

type ExtractState = {
  source: string;
  scan: ExtractScan | null;
  suggestions: ExtractComponent[];
  analysis: ExtractAnalysis | null;
  plan: ExtractPlan | null;
  result: ExtractResult | null;
  runs: ExtractionRun[];
  selection: string[];
  strategy: ExtractStrategy;
  target: string;
  name: string;
  scanning: boolean;
  suggesting: boolean;
  analyzing: boolean;
  planning: boolean;
  executing: boolean;
  loadingRuns: boolean;
  error: string | null;
};

export const extract = $state<ExtractState>({
  source: "",
  scan: null,
  suggestions: [],
  analysis: null,
  plan: null,
  result: null,
  runs: [],
  selection: [],
  strategy: "copy",
  target: "",
  name: "",
  scanning: false,
  suggesting: false,
  analyzing: false,
  planning: false,
  executing: false,
  loadingRuns: false,
  error: null,
});

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

export function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || "component";
}

export function invalidatePlan(): void {
  extract.plan = null;
}

function invalidateAnalysis(): void {
  extract.analysis = null;
  invalidatePlan();
  extract.result = null;
}

export async function scanSource(): Promise<boolean> {
  const source = extract.source.trim();
  if (!source) {
    extract.error = "Enter a repository path to scan.";
    return false;
  }
  extract.scanning = true;
  extract.error = null;
  try {
    const scan = await api.extractScan(source);
    extract.source = scan.source;
    extract.scan = scan;
    extract.selection = [];
    extract.analysis = null;
    extract.plan = null;
    extract.result = null;
    extract.suggestions = [];
    const name = baseName(scan.source);
    extract.name = name;
    extract.target = `~/Projects/${name}`;
    extract.scanning = false;
    void loadSuggestions();
    return true;
  } catch (cause) {
    extract.scan = null;
    extract.error = message(cause);
    extract.scanning = false;
    return false;
  }
}

export async function loadSuggestions(): Promise<void> {
  if (!extract.scan) return;
  extract.suggesting = true;
  try {
    extract.suggestions = await api.extractSuggest(extract.scan.source);
  } catch (cause) {
    extract.suggestions = [];
    notify(message(cause), "danger");
  } finally {
    extract.suggesting = false;
  }
}

export function isSelected(path: string): boolean {
  return extract.selection.includes(path);
}

export function isCovered(path: string): boolean {
  return extract.selection.some(
    (entry) => path === entry || path.startsWith(`${entry}/`),
  );
}

export function toggleSelection(path: string): void {
  if (isSelected(path)) {
    extract.selection = extract.selection.filter((entry) => entry !== path);
  } else {
    extract.selection = extract.selection.filter(
      (entry) => !(entry.startsWith(`${path}/`) || path.startsWith(`${entry}/`)),
    );
    extract.selection = [...extract.selection, path];
  }
  invalidateAnalysis();
}

export function applySuggestion(component: ExtractComponent): void {
  extract.selection = [component.path];
  invalidateAnalysis();
  void analyzeSelection();
}

export function clearSelection(): void {
  extract.selection = [];
  invalidateAnalysis();
}

export async function analyzeSelection(): Promise<boolean> {
  if (!extract.scan) return false;
  if (extract.selection.length === 0) {
    extract.error = "Select at least one file or folder.";
    return false;
  }
  extract.analyzing = true;
  extract.error = null;
  invalidatePlan();
  try {
    extract.analysis = await api.extractAnalyze(extract.scan.source, extract.selection);
    return true;
  } catch (cause) {
    extract.analysis = null;
    extract.error = message(cause);
    return false;
  } finally {
    extract.analyzing = false;
  }
}

export function setStrategy(strategy: ExtractStrategy): void {
  extract.strategy = strategy;
  invalidatePlan();
  extract.result = null;
}

export function planInputsValid(): boolean {
  return (
    extract.selection.length > 0 &&
    extract.target.trim().length > 0 &&
    extract.name.trim().length > 0
  );
}

export async function buildPlan(): Promise<boolean> {
  if (!extract.scan) return false;
  if (!planInputsValid()) {
    extract.error = "Choose a selection, a new folder and a project name.";
    return false;
  }
  extract.planning = true;
  extract.error = null;
  extract.result = null;
  try {
    extract.plan = await api.extractPlan(
      extract.scan.source,
      extract.selection,
      extract.strategy,
      extract.target.trim(),
      extract.name.trim(),
    );
    return true;
  } catch (cause) {
    extract.plan = null;
    extract.error = message(cause);
    return false;
  } finally {
    extract.planning = false;
  }
}

export async function executePlan(): Promise<boolean> {
  const plan = extract.plan;
  if (!plan) return false;
  if (plan.checks.some((check) => !check.ok)) {
    extract.error = "Resolve the failed pre-flight checks before extracting.";
    return false;
  }
  extract.executing = true;
  extract.error = null;
  try {
    const result = await api.extractExecute(
      plan.source,
      plan.selection,
      plan.strategy,
      plan.target,
      plan.name,
      true,
    );
    extract.result = result;
    extract.plan = null;
    extract.selection = [];
    extract.analysis = null;
    notify(
      `${result.strategy === "copy" ? "Copied" : result.strategy === "move" ? "Moved" : "Extracted"} ${result.fileCount} files`,
      "ok",
    );
    await Promise.all([refreshSource(), loadRuns()]);
    return true;
  } catch (cause) {
    extract.error = message(cause);
    notify(message(cause), "danger");
    return false;
  } finally {
    extract.executing = false;
  }
}

async function refreshSource(): Promise<void> {
  if (!extract.source.trim()) return;
  try {
    const scan = await api.extractScan(extract.source);
    extract.scan = scan;
    extract.suggestions = [];
    void loadSuggestions();
  } catch {
    extract.scan = null;
    extract.suggestions = [];
  }
}

export async function loadRuns(): Promise<void> {
  extract.loadingRuns = true;
  try {
    extract.runs = await api.extractRuns();
    extract.error = null;
  } catch (cause) {
    extract.error = message(cause);
  } finally {
    extract.loadingRuns = false;
  }
}

export function resetSource(): void {
  extract.scan = null;
  extract.suggestions = [];
  extract.analysis = null;
  extract.plan = null;
  extract.result = null;
  extract.selection = [];
  extract.error = null;
}
