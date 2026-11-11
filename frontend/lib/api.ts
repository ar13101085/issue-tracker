// Typed client for the Rust API. The types mirror crates/shared/src/lib.rs.

export type Role = "read" | "triage" | "write" | "admin";
export type IssueState = "open" | "closed";
export type StateReason = "completed" | "not_planned" | "duplicate" | "reopened";

export type CurrentUser = { login: string; name: string | null; email: string; is_admin: boolean; email_notifications: boolean };
export type User = { login: string; name: string | null; is_admin: boolean; created_at: string };
export type Repo = {
  owner: string;
  name: string;
  description: string;
  private: boolean;
  created_at: string;
  open_issues_count: number;
  viewer_role: Role | null;
};
export type Collaborator = { login: string; role: Role };
export type Issue = {
  number: number;
  title: string;
  body: string;
  body_html: string;
  state: IssueState;
  state_reason: StateReason | null;
  author: { login: string };
  author_association: Association;
  edit_count: number;
  comments: number;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  labels: Label[];
  assignees: { login: string }[];
  milestone: { number: number; title: string } | null;
  issue_type: string | null;
  locked: boolean;
  lock_reason: string | null;
  pinned: boolean;
  parent: IssueRef | null;
  sub_issues: IssueRef[];
  blocked_by: IssueRef[];
  blocking: IssueRef[];
  related: IssueRef[];
  reactions: ReactionGroup[];
  viewer_can_edit: boolean;
  viewer_can_close: boolean;
  viewer_can_triage: boolean;
  viewer_can_comment: boolean;
  viewer_subscribed: boolean;
};
export type Label = { name: string; color: string; description: string };
export type LabelWithCount = Label & { open_issues: number };
export type ReactionGroup = { content: string; count: number; viewer_reacted: boolean; users: string[] };
export type Milestone = {
  number: number;
  title: string;
  description: string;
  state: IssueState;
  due_on: string | null;
  open_issues: number;
  closed_issues: number;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
};
export type Template = { name: string; about: string; title: string; body: string; labels: string[]; issue_type: string | null };
export type Attachment = { url: string; name: string; content_type: string; size: number; markdown: string };
export type Notification = {
  id: string;
  repo: string;
  issue: IssueRef;
  reason: string;
  unread: boolean;
  done: boolean;
  last_actor: { login: string };
  updated_at: string;
};
export type NotificationList = { unread_count: number; total_count: number; items: Notification[] };

export const ISSUE_TYPES = ["Bug", "Feature", "Task"] as const;
export const LOCK_REASONS = ["off-topic", "too heated", "resolved", "spam"] as const;
export const REACTIONS: { content: string; emoji: string; label: string }[] = [
  { content: "+1", emoji: "👍", label: "Thumbs up" },
  { content: "-1", emoji: "👎", label: "Thumbs down" },
  { content: "laugh", emoji: "😄", label: "Laugh" },
  { content: "hooray", emoji: "🎉", label: "Hooray" },
  { content: "confused", emoji: "😕", label: "Confused" },
  { content: "heart", emoji: "❤️", label: "Heart" },
  { content: "rocket", emoji: "🚀", label: "Rocket" },
  { content: "eyes", emoji: "👀", label: "Eyes" },
];
export type IssueList = {
  total_count: number;
  open_count: number;
  closed_count: number;
  page: number;
  per_page: number;
  items: Issue[];
};

export type Association = "owner" | "collaborator" | "none";
export type Comment = {
  id: string;
  author: { login: string };
  author_association: Association;
  body: string;
  body_html: string;
  created_at: string;
  updated_at: string;
  edit_count: number;
  reactions: ReactionGroup[];
  viewer_can_edit: boolean;
  viewer_can_delete: boolean;
  /** Why the comment is hidden; null when it is shown. */
  minimized: string | null;
  viewer_can_minimize: boolean;
};
export type SavedView = { id: string; name: string; description: string; query: string; shared: boolean; viewer_can_edit: boolean; created_at: string };
export const HIDE_REASONS = ["abuse", "spam", "off-topic", "outdated", "duplicate", "resolved"] as const;
export type Edit = { editor: { login: string }; edited_at: string; body: string };
export type EventKind =
  | "closed"
  | "reopened"
  | "renamed"
  | "cross_referenced"
  | "labeled"
  | "unlabeled"
  | "assigned"
  | "unassigned"
  | "milestoned"
  | "demilestoned"
  | "locked"
  | "unlocked"
  | "pinned"
  | "unpinned"
  | "transferred"
  | "sub_issue_added"
  | "sub_issue_removed"
  | "parent_added"
  | "parent_removed"
  | "blocked_by_added"
  | "blocked_by_removed"
  | "blocking_added"
  | "blocking_removed"
  | "related_added"
  | "related_removed"
  | "type_changed";
export type LabelRef = { name: string; color: string };
export type IssueRef = { number: number; title: string; state: IssueState; state_reason: StateReason | null };
export type TimelineEvent = {
  id: string;
  event: EventKind;
  actor: { login: string };
  created_at: string;
  state_reason: StateReason | null;
  rename: { from: string; to: string } | null;
  source: IssueRef | null;
  label: LabelRef | null;
  assignee: { login: string } | null;
  milestone: string | null;
  detail: string | null;
};
export type TimelineItem = ({ type: "comment" } & Comment) | ({ type: "event" } & TimelineEvent);

export type FieldError = { field: string; message: string };

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public errors: FieldError[] = [],
  ) {
    super(message);
  }
  /** Message for one form field, if the server named it. */
  field(name: string): string | undefined {
    return this.errors.find((e) => e.field === name)?.message;
  }
  /** One line to show when there is no per-field place for it. */
  get summary(): string {
    return this.errors[0]?.message ?? this.message;
  }
}

export async function api<T>(path: string, init: { method?: string; body?: unknown } = {}): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`/api${path}`, {
      method: init.method ?? "GET",
      headers: init.body === undefined ? undefined : { "Content-Type": "application/json" },
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      credentials: "same-origin",
    });
  } catch {
    throw new ApiError(0, "Could not reach the server. Check your connection and try again.");
  }
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(res.status, data?.message ?? `Request failed (${res.status})`, data?.errors ?? []);
  return data as T;
}

export const ROLE_RANK: Record<Role, number> = { read: 0, triage: 1, write: 2, admin: 3 };

export function hasRole(repo: Repo | undefined, min: Role): boolean {
  return !!repo?.viewer_role && ROLE_RANK[repo.viewer_role] >= ROLE_RANK[min];
}

export function asApiError(e: unknown): ApiError {
  return e instanceof ApiError ? e : new ApiError(0, "Something went wrong. Try again.");
}

/** Uploads a file for use in a comment. The body is the file itself. */
export async function upload(repo: string, file: File): Promise<Attachment> {
  let res: Response;
  try {
    res = await fetch(`/api/repos/${repo}/attachments?name=${encodeURIComponent(file.name || "file")}`, {
      method: "POST",
      body: file,
      credentials: "same-origin",
    });
  } catch {
    throw new ApiError(0, "Could not reach the server. Check your connection and try again.");
  }
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    const tooBig = res.status === 413 ? "The file is larger than 10 MB" : undefined;
    throw new ApiError(res.status, tooBig ?? data?.message ?? `Upload failed (${res.status})`, data?.errors ?? []);
  }
  return data as Attachment;
}

// ---- Projects ----
export type FieldKind = "single_select" | "text" | "number" | "date";
export type FieldOption = { id: string; name: string; color: string };
export type ProjectField = { id: string; name: string; kind: FieldKind; options: FieldOption[] };
export type ProjectSummary = { number: number; title: string; description: string; closed: boolean; item_count: number; created_at: string; updated_at: string };
export type ProjectItem = { id: string; issue: Issue; values: Record<string, string | number>; position: number };
export type Project = ProjectSummary & { fields: ProjectField[]; items: ProjectItem[]; viewer_can_admin: boolean; viewer_can_edit: boolean };
export type IssueProject = { number: number; title: string; item_id: string; status: FieldOption | null };
export const STATUS_FIELD = "status";
export const OPTION_COLORS = ["gray", "blue", "green", "yellow", "orange", "red", "pink", "purple"] as const;
