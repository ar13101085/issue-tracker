// The issue search box syntax: `is:issue state:open label:"good first issue" sort:created-asc words`.
// The server parses the same string (crates/shared/src/query.rs); this copy drives the UI.

import type { IssueState } from "@/lib/api";

export type SortKey = "created-desc" | "created-asc" | "comments-desc" | "updated-desc";

export const SORTS: { key: SortKey; label: string }[] = [
  { key: "created-desc", label: "Newest" },
  { key: "created-asc", label: "Oldest" },
  { key: "comments-desc", label: "Most commented" },
  { key: "updated-desc", label: "Recently updated" },
];

export const DEFAULT_QUERY = "is:issue state:open";

export type NoField = "label" | "assignee" | "milestone" | "type";

export type ParsedQuery = {
  state: IssueState | "all";
  sort: SortKey;
  labels: string[];
  no: NoField[];
  author: string | null;
  assignee: string | null;
  milestone: string | null;
  type: string | null;
  text: string;
};

/** Splits on spaces, keeping `"quoted phrases"` (also after `key:`) together. */
export function tokenize(q: string): string[] {
  const out: string[] = [];
  let current = "";
  let quoted = false;
  for (const c of q) {
    if (c === '"') {
      quoted = !quoted;
      current += c;
    } else if (/\s/.test(c) && !quoted) {
      if (current) out.push(current);
      current = "";
    } else {
      current += c;
    }
  }
  if (current) out.push(current);
  return out;
}

const unquote = (v: string) => v.replace(/^"+|"+$/g, "");
const quoteIfNeeded = (v: string) => (/\s/.test(v) ? `"${v}"` : v);

function split(token: string): [string, string] | null {
  const i = token.indexOf(":");
  return i > 0 ? [token.slice(0, i), unquote(token.slice(i + 1))] : null;
}

export function parseQuery(q: string): ParsedQuery {
  const out: ParsedQuery = { state: "all", sort: "created-desc", labels: [], no: [], author: null, assignee: null, milestone: null, type: null, text: "" };
  const words: string[] = [];
  for (const token of tokenize(q)) {
    const pair = split(token);
    const [key, value] = pair ?? ["", ""];
    const lower = value.toLowerCase();
    if ((key === "state" || key === "is") && (lower === "open" || lower === "closed")) out.state = lower;
    else if (key === "is" && ["issue", "blocked", "blocking"].includes(lower)) continue;
    else if (key === "sort" && SORTS.some((s) => s.key === lower)) out.sort = lower as SortKey;
    else if (key === "label" && value) out.labels.push(value);
    else if (key === "no" && ["label", "assignee", "milestone", "type"].includes(lower)) out.no.push(lower as NoField);
    else if (key === "author" && value) out.author = value;
    else if (key === "assignee" && value) out.assignee = value;
    else if (key === "milestone" && value) out.milestone = value;
    else if (key === "type" && value) out.type = value;
    else if (!pair || !["-label", "in", "mentions", "commenter", "involves"].includes(key)) words.push(unquote(token));
  }
  out.text = words.join(" ");
  return out;
}

function isToken(token: string, key: string, value?: string): boolean {
  const pair = split(token);
  if (!pair || pair[0] !== key) return false;
  return value === undefined || pair[1].toLowerCase() === value.toLowerCase();
}

/** Qualifiers first, free words last: how GitHub keeps the box tidy. */
function join(tokens: string[], added: string[] = []): string {
  const qualifiers = tokens.filter((t) => split(t));
  const words = tokens.filter((t) => !split(t));
  return [...qualifiers, ...added, ...words].join(" ");
}

/** Sets a single-valued qualifier (`state`, `sort`, `author`, ...); `null` removes it. */
export function setToken(q: string, key: string, value: string | null): string {
  const kept = tokenize(q).filter((t) => {
    if (isToken(t, key)) return false;
    if (key === "state" && (isToken(t, "is", "open") || isToken(t, "is", "closed"))) return false;
    // Choosing a value clears the matching "no:" and the other way round.
    if (value !== null && isToken(t, "no", key)) return false;
    return true;
  });
  return join(kept, value === null ? [] : [`${key}:${quoteIfNeeded(value)}`]);
}

/** Adds or removes one value of a repeatable qualifier (`label`). */
export function toggleToken(q: string, key: string, value: string): string {
  const tokens = tokenize(q);
  if (tokens.some((t) => isToken(t, key, value))) return join(tokens.filter((t) => !isToken(t, key, value)));
  return join(
    tokens.filter((t) => !isToken(t, "no", key)),
    [`${key}:${quoteIfNeeded(value)}`],
  );
}

/** Toggles `no:label` and friends, dropping any value of that qualifier. */
export function toggleNo(q: string, field: NoField): string {
  const tokens = tokenize(q);
  if (tokens.some((t) => isToken(t, "no", field))) return join(tokens.filter((t) => !isToken(t, "no", field)));
  return join(
    tokens.filter((t) => !isToken(t, field)),
    [`no:${field}`],
  );
}

/** Link to the issue list filtered by one extra qualifier. */
export function searchHref(base: string, q: string, page = 1): string {
  const params = new URLSearchParams();
  if (q !== DEFAULT_QUERY) params.set("q", q);
  if (page > 1) params.set("page", String(page));
  const qs = params.toString();
  return qs ? `${base}?${qs}` : base;
}
