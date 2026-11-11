"use client";

import {
  CrossReferenceIcon,
  IssueClosedIcon,
  IssueReopenedIcon,
  IssueDraftIcon,
  KebabHorizontalIcon,
  LockIcon,
  MilestoneIcon,
  PencilIcon,
  PersonIcon,
  PinIcon,
  SkipIcon,
  TagIcon,
  TrashIcon,
  IssueTrackedByIcon,
  IssueTracksIcon,
  ArrowRightIcon,
  TriangleDownIcon,
  UnlockIcon,
  CopyIcon,
  QuoteIcon,
} from "@primer/octicons-react";
import { type ReactNode, useState } from "react";
import useSWR from "swr";
import { MarkdownEditor } from "@/components/markdown/editor";
import { MarkdownBody } from "@/components/markdown/markdown-body";
import { ActionDivider, ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { Avatar } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Flash } from "@/components/ui/form";
import { IssueLabel } from "@/components/ui/issue-label";
import { IssueStateIcon, RelativeTime } from "@/components/ui/misc";
import { api, asApiError, type Association, type Edit, type TimelineEvent } from "@/lib/api";
import { Link } from "@/lib/router";
import { cn } from "@/lib/utils";

function UserLink({ login, className }: { login: string; className?: string }) {
  return (
    <Link href={`/${login}`} className={cn("font-semibold text-fg hover:text-fg-accent hover:underline", className)}>
      {login}
    </Link>
  );
}

function Badge({ children, title }: { children: ReactNode; title: string }) {
  return (
    <span title={title} className="hidden h-5 items-center rounded-full border border-line px-[7px] text-xs font-medium text-fg-muted sm:inline-flex">
      {children}
    </span>
  );
}

function EditHistory({ path, count }: { path: string; count: number }) {
  const [open, setOpen] = useState(false);
  const [shown, setShown] = useState<Edit | null>(null);
  const { data } = useSWR<Edit[]>(open ? path : null, api);
  return (
    <>
      <ActionMenu
        align="end"
        width={280}
        trigger={
          <button type="button" onClick={() => setOpen(true)} className="inline-flex shrink-0 cursor-pointer items-center text-xs text-fg-muted hover:text-fg-accent">
            Edited
            <TriangleDownIcon size={16} />
          </button>
        }
      >
        <div className="px-2 py-1.5 text-xs font-semibold text-fg-muted">
          Edited {count} time{count === 1 ? "" : "s"}
        </div>
        {!data && <div className="px-2 py-1.5 text-fg-muted">Loading…</div>}
        {data?.map((e, i) => (
          <ActionItem key={i} leading={<Avatar login={e.editor.login} size={20} />} onSelect={() => setShown(e)}>
            <span className="min-w-0 truncate">
              <span className="font-semibold">{e.editor.login}</span> <span className="text-fg-muted">edited <RelativeTime iso={e.edited_at} /></span>
            </span>
          </ActionItem>
        ))}
      </ActionMenu>
      <Dialog
        open={!!shown}
        onClose={() => setShown(null)}
        width={640}
        title="Earlier version"
        subtitle={shown && <>Replaced by {shown.editor.login} <RelativeTime iso={shown.edited_at} /></>}
      >
        <pre className="font-mono text-xs break-words whitespace-pre-wrap">{shown?.body || "(empty)"}</pre>
      </Dialog>
    </>
  );
}

export type CommentBoxProps = {
  anchor: string;
  repo: string;
  author: string;
  association: Association;
  /** The comment's author also opened the issue. */
  isIssueAuthor: boolean;
  verb: "opened" | "commented";
  createdAt: string;
  body: string;
  bodyHtml: string;
  editCount: number;
  editsPath: string;
  canEdit: boolean;
  onSave: (body: string) => Promise<void>;
  onDelete?: () => Promise<void>;
  onQuote?: (body: string) => void;
  /** Rendered under the body: reactions. */
  footer?: ReactNode;
};

export function CommentBox(p: CommentBoxProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [copied, setCopied] = useState(false);

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
      setEditing(false);
      setConfirmDelete(false);
    } catch (err) {
      setError(asApiError(err).summary);
    } finally {
      setBusy(false);
    }
  }

  function copyLink() {
    const url = `${window.location.origin}${window.location.pathname}#${p.anchor}`;
    navigator.clipboard?.writeText(url).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }

  return (
    <div id={p.anchor} className="relative scroll-mt-20 py-2">
      <div className="min-w-0">
        {editing ? (
          <div className="flex flex-col gap-2 bg-canvas">
            {error && <Flash>{error}</Flash>}
            <MarkdownEditor value={draft} onChange={setDraft} repo={p.repo} autoFocus onSubmit={() => run(() => p.onSave(draft))} />
            <div className="flex justify-end gap-2">
              <Button onClick={() => setEditing(false)}>Cancel</Button>
              <Button variant="primary" disabled={busy || (p.verb === "commented" && !draft.trim())} onClick={() => run(() => p.onSave(draft))}>
                {p.verb === "opened" ? "Save" : "Update comment"}
              </Button>
            </div>
          </div>
        ) : (
          <article className="rounded-md border border-line bg-canvas">
            <header className="flex min-h-10 items-center gap-2 rounded-t-md border-b border-line bg-canvas-muted py-1 pr-2 pl-2 text-sm text-fg-muted">
              <Link href={`/${p.author}`} className="shrink-0">
                <Avatar login={p.author} size={24} />
              </Link>
              <span className="min-w-0 flex-1">
                <UserLink login={p.author} /> {p.verb}{" "}
                <a href={`#${p.anchor}`} className="hover:text-fg-accent hover:underline">
                  <RelativeTime iso={p.createdAt} />
                </a>
              </span>
              {p.editCount > 0 && <EditHistory path={p.editsPath} count={p.editCount} />}
              {p.association === "owner" && <Badge title="This user is the owner of this repository.">Owner</Badge>}
              {p.association === "collaborator" && <Badge title="This user has been invited to collaborate on this repository.">Collaborator</Badge>}
              {p.isIssueAuthor && p.verb === "commented" && <Badge title="This user is the author of this issue.">Author</Badge>}
              <ActionMenu
                trigger={
                  <button type="button" aria-label="Comment actions" className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
                    <KebabHorizontalIcon size={16} />
                  </button>
                }
              >
                <ActionItem leading={<CopyIcon size={16} />} onSelect={copyLink}>
                  Copy link
                </ActionItem>
                {p.onQuote && (
                  <ActionItem leading={<QuoteIcon size={16} />} onSelect={() => p.onQuote?.(p.body)}>
                    Quote reply
                  </ActionItem>
                )}
                {p.canEdit && (
                  <>
                    <ActionDivider />
                    <ActionItem
                      leading={<PencilIcon size={16} />}
                      onSelect={() => {
                        setDraft(p.body);
                        setEditing(true);
                      }}
                    >
                      Edit
                    </ActionItem>
                    {p.onDelete && (
                      <ActionItem danger leading={<TrashIcon size={16} />} onSelect={() => setConfirmDelete(true)}>
                        Delete
                      </ActionItem>
                    )}
                  </>
                )}
              </ActionMenu>
            </header>
            <div className="p-4">
              <MarkdownBody html={p.bodyHtml} />
              {p.footer}
            </div>
          </article>
        )}
        {copied && <p className="mt-1 text-xs text-fg-muted">Link copied.</p>}
      </div>
      <Dialog
        open={confirmDelete}
        onClose={() => setConfirmDelete(false)}
        title="Delete comment"
        footer={
          <>
            <Button onClick={() => setConfirmDelete(false)}>Cancel</Button>
            <Button variant="danger" disabled={busy} onClick={() => p.onDelete && run(p.onDelete)}>
              Delete
            </Button>
          </>
        }
      >
        {error && <Flash className="mb-3">{error}</Flash>}
        Are you sure you want to delete this comment? This cannot be undone.
      </Dialog>
    </div>
  );
}

function IssueLine({ base, issue }: { base: string; issue: NonNullable<TimelineEvent["source"]> }) {
  return (
    <div className="mt-2 flex items-start gap-2 text-sm">
      <span className="flex h-5 items-center">
        <IssueStateIcon issue={issue} />
      </span>
      <Link href={`${base}/${issue.number}`} className="min-w-0 font-semibold break-words text-fg hover:text-fg-accent">
        {issue.title} <span className="font-normal text-fg-muted">#{issue.number}</span>
      </Link>
    </div>
  );
}

/** Icon, badge color and sentence for each kind of event. */
function describe(e: TimelineEvent, labelHref: (name: string) => string): { icon: ReactNode; tone?: "done" | "success" | "neutral"; text: ReactNode } {
  const strong = (t: string) => <span className="font-semibold text-fg">{t}</span>;
  switch (e.event) {
    case "closed":
      if (e.state_reason === "not_planned") return { icon: <SkipIcon size={16} />, tone: "neutral", text: <>closed this as not planned</> };
      if (e.state_reason === "duplicate") return { icon: <SkipIcon size={16} />, tone: "neutral", text: <>closed this as a duplicate</> };
      return { icon: <IssueClosedIcon size={16} />, tone: "done", text: <>closed this as completed</> };
    case "reopened":
      return { icon: <IssueReopenedIcon size={16} />, tone: "success", text: <>reopened this</> };
    case "renamed":
      return {
        icon: <PencilIcon size={16} />,
        text: (
          <>
            changed the title <del className="text-fg">{e.rename?.from}</del> {strong(e.rename?.to ?? "")}
          </>
        ),
      };
    case "cross_referenced":
      return { icon: <CrossReferenceIcon size={16} />, text: <>mentioned this</> };
    case "labeled":
    case "unlabeled":
      return {
        icon: <TagIcon size={16} />,
        text: (
          <>
            {e.event === "labeled" ? "added" : "removed"} {e.label && <IssueLabel label={e.label} href={labelHref(e.label.name)} />}
          </>
        ),
      };
    case "assigned":
    case "unassigned": {
      const self = e.assignee?.login === e.actor.login;
      const verb = e.event === "assigned" ? (self ? "self-assigned this" : "assigned") : self ? "removed their assignment" : "unassigned";
      return { icon: <PersonIcon size={16} />, text: <>{verb} {!self && e.assignee && <UserLink login={e.assignee.login} />}</> };
    }
    case "milestoned":
      return { icon: <MilestoneIcon size={16} />, text: <>added this to the {strong(e.milestone ?? "")} milestone</> };
    case "demilestoned":
      return { icon: <MilestoneIcon size={16} />, text: <>removed this from the {strong(e.milestone ?? "")} milestone</> };
    case "locked":
      return {
        icon: <LockIcon size={16} />,
        text: <>locked {e.detail ? <>as {strong(e.detail)} </> : null}and limited conversation to collaborators</>,
      };
    case "unlocked":
      return { icon: <UnlockIcon size={16} />, text: <>unlocked this conversation</> };
    case "pinned":
      return { icon: <PinIcon size={16} />, text: <>pinned this issue</> };
    case "unpinned":
      return { icon: <PinIcon size={16} />, text: <>unpinned this issue</> };
    case "transferred":
      return { icon: <ArrowRightIcon size={16} />, text: <>transferred this issue from {strong(e.detail ?? "another repository")}</> };
    case "sub_issue_added":
      return { icon: <IssueTracksIcon size={16} />, text: <>added a sub-issue</> };
    case "sub_issue_removed":
      return { icon: <IssueTracksIcon size={16} />, text: <>removed a sub-issue</> };
    case "parent_added":
      return { icon: <IssueTrackedByIcon size={16} />, text: <>added a parent issue</> };
    case "parent_removed":
      return { icon: <IssueTrackedByIcon size={16} />, text: <>removed a parent issue</> };
    case "type_changed":
      return {
        icon: <IssueDraftIcon size={16} />,
        text: e.detail ? <>added the {strong(e.detail)} issue type</> : <>removed the issue type</>,
      };
  }
}

export function EventRow({ event, base }: { event: TimelineEvent; base: string }) {
  const d = describe(event, (name) => `${base}?q=${encodeURIComponent(`is:issue state:open label:"${name}"`)}`);
  const tone = {
    done: "bg-done-emphasis text-fg-on-emphasis",
    success: "bg-success-emphasis text-fg-on-emphasis",
    neutral: "bg-[var(--bgColor-neutral-emphasis)] text-fg-on-emphasis",
  };
  return (
    <div className="relative flex gap-2 py-2 pl-5">
      <span
        className={cn(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-2 border-canvas",
          d.tone ? tone[d.tone] : "bg-[var(--timelineBadge-bgColor)] text-fg-muted",
        )}
      >
        {d.icon}
      </span>
      <div className="min-w-0 flex-1 pt-1.5 text-sm text-fg-muted">
        <Avatar login={event.actor.login} size={20} className="mr-1 inline-block align-text-bottom" />
        <UserLink login={event.actor.login} /> {d.text}{" "}
        <RelativeTime iso={event.created_at} />
        {event.source && <IssueLine base={base} issue={event.source} />}
      </div>
    </div>
  );
}
