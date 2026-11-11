"use client";

import {
  ArrowRightIcon,
  CheckIcon,
  IssueClosedIcon,
  IssueReopenedIcon,
  LockIcon,
  PinIcon,
  SkipIcon,
  TrashIcon,
  TriangleDownIcon,
  UnlockIcon,
} from "@primer/octicons-react";
import { type FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
import useSWR, { useSWRConfig } from "swr";
import { TypeBadge } from "@/components/issue/pickers";
import { Reactions } from "@/components/issue/reactions";
import { type Metadata, MetadataSections, ParticipantsSection, ProjectsSection, SidebarSection, SubscribeSection } from "@/components/issue/sidebar";
import { SubIssues } from "@/components/issue/sub-issues";
import { CommentBox, EventRow } from "@/components/issue/timeline";
import { type EditorHandle, MarkdownEditor } from "@/components/markdown/editor";
import { quote } from "@/components/markdown/textarea-tools";
import { Container, PageError } from "@/components/pages/repos";
import { ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { Avatar } from "@/components/ui/avatar";
import { Button, buttonClass } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, inputClass, TextInput } from "@/components/ui/form";
import { IssueStateIcon, StateLabel, useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, type Comment, hasRole, type Issue, LOCK_REASONS, type Milestone, type StateReason, type Template, type TimelineItem } from "@/lib/api";
import { Link, Redirect, navigate } from "@/lib/router";
import { useRepo, useSession } from "@/lib/session";
import { cn } from "@/lib/utils";

/** Body of a create or update request for the organising fields. */
function metadataBody(change: Partial<Metadata>, milestones: Milestone[] | undefined): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  if (change.assignees) body.assignees = change.assignees;
  if (change.labels) body.labels = change.labels;
  if (change.type !== undefined) body.issue_type = change.type;
  if (change.milestone !== undefined) body.milestone = change.milestone === null ? null : (milestones?.find((m) => m.title === change.milestone)?.number ?? null);
  return body;
}

export function NewIssuePage({ owner, repo }: { owner: string; repo: string }) {
  useTitle(`New issue · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const base = `/${owner}/${repo}/issues`;
  const { user } = useSession();
  const { data: repoData, error: repoError } = useRepo(owner, repo);
  const templates = useSWR<Template[]>(`/repos/${full}/issue-templates`, api);
  const milestones = useSWR<Milestone[]>(`/repos/${full}/milestones?state=all`, api);
  const { mutate } = useSWRConfig();
  // `undefined`: still choosing. `null`: blank issue.
  const [template, setTemplate] = useState<Template | null | undefined>(undefined);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [meta, setMeta] = useState<Metadata>({ assignees: [], labels: [], type: null, milestone: null });
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  if (repoError) return <PageError error={repoError} />;
  if (user === undefined || !templates.data) return null;
  if (user === null) return <Redirect to={`/login?return_to=${encodeURIComponent(`${base}/new`)}`} />;
  const canTriage = hasRole(repoData, "triage");

  function choose(t: Template | null) {
    setTemplate(t);
    setTitle(t?.title ?? "");
    setBody(t?.body ?? "");
    setMeta({ assignees: [], labels: t?.labels ?? [], type: t?.issue_type ?? null, milestone: null });
  }

  if (template === undefined && templates.data.length > 0) {
    return (
      <Container narrow>
        <h1 className="text-xl font-semibold">Create new issue</h1>
        <ul className="mt-4 divide-y divide-line-muted rounded-md border border-line">
          {[...templates.data, null].map((t) => (
            <li key={t?.name ?? "__blank__"} className="flex items-center gap-4 p-4">
              <div className="min-w-0 flex-1">
                <p className="font-semibold">{t?.name ?? "Blank issue"}</p>
                <p className="text-sm text-fg-muted">{t ? t.about : "Create a new issue from scratch"}</p>
              </div>
              <Button variant={t ? "primary" : "default"} onClick={() => choose(t)}>
                Get started
              </Button>
            </li>
          ))}
        </ul>
      </Container>
    );
  }

  async function create() {
    if (busy || !title.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const issue = await api<Issue>(`/repos/${full}/issues`, { method: "POST", body: { title, body, ...metadataBody(meta, milestones.data) } });
      mutate(`/repos/${full}`);
      navigate(`${base}/${issue.number}`);
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  function submit(e: FormEvent) {
    e.preventDefault();
    create();
  }

  return (
    <Container>
      <div className="flex flex-col gap-6 lg:flex-row">
        <div className="flex min-w-0 flex-1 gap-4">
          <Avatar login={user.login} size={40} className="hidden md:block" />
          <form onSubmit={submit} className="flex min-w-0 flex-1 flex-col gap-4">
            <h1 className="text-xl font-semibold">
              Create new issue{template ? <span className="font-normal text-fg-muted"> · {template.name}</span> : null}
            </h1>
            {error && (error.errors.length === 0 || !error.field("title")) && <Flash>{error.summary}</Flash>}
            <Field label="Add a title" htmlFor="issue-title" required error={error?.field("title")}>
              <TextInput id="issue-title" placeholder="Title" autoFocus maxLength={256} aria-invalid={!!error?.field("title")} value={title} onChange={(e) => setTitle(e.target.value)} />
            </Field>
            <div className="flex flex-col gap-1">
              <label htmlFor="issue-body" className="text-sm font-semibold">
                Add a description
              </label>
              <MarkdownEditor id="issue-body" value={body} onChange={setBody} repo={full} placeholder="Type your description here…" minHeight={200} onSubmit={create} />
            </div>
            <div className="flex justify-end gap-2">
              <Button href={base}>Cancel</Button>
              <Button type="submit" variant="primary" disabled={busy || !title.trim()}>
                {busy ? "Creating…" : "Create"}
              </Button>
            </div>
          </form>
        </div>
        {canTriage && (
          <aside className="w-full shrink-0 lg:w-[296px]">
            <MetadataSections repo={full} value={meta} canEdit viewer={user.login} onChange={(c) => setMeta((m) => ({ ...m, ...c }))} />
          </aside>
        )}
      </div>
    </Container>
  );
}

/** Slim bar that takes over once the title has scrolled out of view. */
function StickyHeader({ issue, show }: { issue: Issue; show: boolean }) {
  return (
    <div
      aria-hidden={!show}
      className={cn("fixed inset-x-0 top-0 z-30 border-b border-line bg-canvas shadow-[var(--shadow-resting-small)] transition-transform", show ? "translate-y-0" : "-translate-y-full")}
    >
      <div className="mx-auto flex max-w-[1280px] items-center gap-2 px-4 py-2 md:px-6 lg:px-8">
        <StateLabel issue={issue} />
        <span className="min-w-0 truncate text-sm font-semibold">
          {issue.title} <span className="font-normal text-fg-muted">#{issue.number}</span>
        </span>
      </div>
    </div>
  );
}

const CLOSE_REASONS: { reason: StateReason; label: string; hint: string; icon: ReactNode }[] = [
  { reason: "completed", label: "Close as completed", hint: "Done, closed, fixed, resolved", icon: <IssueClosedIcon size={16} className="text-fg-done" /> },
  { reason: "not_planned", label: "Close as not planned", hint: "Won't fix, can't repro, stale", icon: <SkipIcon size={16} className="text-fg-muted" /> },
  { reason: "duplicate", label: "Close as duplicate", hint: "Duplicate of another issue", icon: <SkipIcon size={16} className="text-fg-muted" /> },
];

type Modal = "lock" | "transfer" | "delete" | null;

export function IssuePage({ owner, repo, number }: { owner: string; repo: string; number: number }) {
  const full = `${owner}/${repo}`;
  const repoPath = `/repos/${full}`;
  const key = `${repoPath}/issues/${number}`;
  const base = `/${owner}/${repo}/issues`;
  const { user } = useSession();
  const { data: repoData } = useRepo(owner, repo);
  const { data: issue, error, mutate } = useSWR<Issue, ApiError>(key, api);
  const timeline = useSWR<TimelineItem[], ApiError>(`${key}/timeline`, api);
  const milestones = useSWR<Milestone[]>(`${repoPath}/milestones?state=all`, api);
  const { mutate: mutateAny } = useSWRConfig();

  const [editingTitle, setEditingTitle] = useState(false);
  const [title, setTitle] = useState("");
  const [comment, setComment] = useState("");
  const [closeReason, setCloseReason] = useState<StateReason>("completed");
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [modal, setModal] = useState<Modal>(null);
  const [lockReason, setLockReason] = useState("");
  const [destination, setDestination] = useState("");
  const [modalError, setModalError] = useState<string | null>(null);
  const editor = useRef<EditorHandle>(null);
  const titleRef = useRef<HTMLDivElement>(null);
  const [stuck, setStuck] = useState(false);
  useTitle(issue ? `${issue.title} · Issue #${number} · ${full}` : `Issue #${number} · ${full}`);

  useEffect(() => {
    const el = titleRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(([entry]) => setStuck(!entry.isIntersecting && entry.boundingClientRect.top < 0));
    observer.observe(el);
    return () => observer.disconnect();
  }, [issue?.number]);

  // Jump to the comment named in the address bar once it exists.
  const scrolled = useRef(false);
  useEffect(() => {
    if (scrolled.current || !timeline.data || !window.location.hash) return;
    document.getElementById(window.location.hash.slice(1))?.scrollIntoView();
    scrolled.current = true;
  }, [timeline.data]);

  if (error) return <PageError error={error} />;
  if (!issue) return null;

  const refresh = () => Promise.all([mutate(), timeline.mutate(), mutateAny(repoPath), mutateAny((k) => typeof k === "string" && k.startsWith(`${repoPath}/milestones`))]);

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setActionError(null);
    try {
      await action();
      await refresh();
    } catch (err) {
      setActionError(asApiError(err).summary);
      throw err;
    } finally {
      setBusy(false);
    }
  }
  const quiet = (action: () => Promise<unknown>) => run(action).catch(() => {});
  const patchIssue = (body: Record<string, unknown>) => quiet(() => api<Issue>(key, { method: "PATCH", body }));

  async function postComment(then?: Record<string, unknown>) {
    await quiet(async () => {
      if (comment.trim()) await api<Comment>(`${key}/comments`, { method: "POST", body: { body: comment } });
      if (then) await api<Issue>(key, { method: "PATCH", body: then });
      setComment("");
    });
  }

  function saveTitle(e: FormEvent) {
    e.preventDefault();
    patchIssue({ title }).then(() => setEditingTitle(false));
  }

  /** Runs a dialog action; errors stay inside the dialog. */
  async function inModal(action: () => Promise<unknown>, after?: () => void) {
    setBusy(true);
    setModalError(null);
    try {
      await action();
      setModal(null);
      if (after) after();
      else await refresh();
    } catch (err) {
      setModalError(asApiError(err).summary);
    } finally {
      setBusy(false);
    }
  }

  const items = timeline.data ?? [];
  const participants = [...new Set([issue.author.login, ...items.flatMap((i) => (i.type === "comment" ? [i.author.login] : []))])];
  const chosen = CLOSE_REASONS.find((r) => r.reason === closeReason) ?? CLOSE_REASONS[0];
  const hasText = comment.trim().length > 0;
  const canWrite = hasRole(repoData, "write");
  const isAdmin = hasRole(repoData, "admin");
  const meta: Metadata = { assignees: issue.assignees.map((a) => a.login), labels: issue.labels.map((l) => l.name), type: issue.issue_type, milestone: issue.milestone?.title ?? null };
  const sideAction = "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm font-semibold hover:bg-control-transparent-hover";

  return (
    <Container>
      <StickyHeader issue={issue} show={stuck} />
      <div ref={titleRef}>
        {editingTitle ? (
          <form onSubmit={saveTitle} className="flex flex-wrap gap-2">
            <TextInput aria-label="Title" autoFocus maxLength={256} className="min-w-0 flex-1 basis-full md:basis-0" value={title} onChange={(e) => setTitle(e.target.value)} />
            <Button type="submit" variant="primary" disabled={busy || !title.trim()}>
              Save
            </Button>
            <Button onClick={() => setEditingTitle(false)}>Cancel</Button>
          </form>
        ) : (
          <div className="flex flex-col-reverse items-start gap-2 md:flex-row">
            <h1 className="min-w-0 flex-1 text-[26px] leading-tight font-normal break-words md:text-[32px] md:leading-10">
              {issue.title} <span className="text-fg-muted">#{issue.number}</span>
            </h1>
            <div className="flex shrink-0 gap-2">
              {issue.viewer_can_edit && (
                <Button
                  onClick={() => {
                    setTitle(issue.title);
                    setEditingTitle(true);
                  }}
                >
                  Edit
                </Button>
              )}
              <Button variant="primary" href={`${base}/new`}>
                New issue
              </Button>
            </div>
          </div>
        )}
        <div className="mt-3 flex flex-wrap items-center gap-2 text-sm text-fg-muted">
          <StateLabel issue={issue} />
          {issue.issue_type && <TypeBadge type={issue.issue_type} className="h-8 px-3 text-sm" />}
          {issue.pinned && (
            <span className="flex items-center gap-1">
              <PinIcon size={16} />
              Pinned
            </span>
          )}
        </div>
      </div>

      {actionError && (
        <Flash className="mt-4" onDismiss={() => setActionError(null)}>
          {actionError}
        </Flash>
      )}

      <div className="mt-4 flex flex-col gap-6 lg:flex-row">
        <div className="min-w-0 flex-1">
          <div className="relative">
            <div aria-hidden className="absolute top-4 bottom-0 left-[35px] w-0.5 bg-line-muted" />
            <CommentBox
              anchor={`issue-${issue.number}`}
              repo={full}
              author={issue.author.login}
              association={issue.author_association}
              isIssueAuthor
              verb="opened"
              createdAt={issue.created_at}
              body={issue.body}
              bodyHtml={issue.body_html}
              editCount={issue.edit_count}
              editsPath={`${key}/edits`}
              canEdit={issue.viewer_can_edit}
              onSave={(body) => api<Issue>(key, { method: "PATCH", body: { body } }).then(() => void refresh())}
              onQuote={issue.viewer_can_comment ? (text) => editor.current?.insert(quote(text)) : undefined}
              footer={
                <>
                  <SubIssues
                    repo={full}
                    issue={issue}
                    onAdd={(n) => quiet(() => api(`${key}/sub_issues`, { method: "POST", body: { number: n } }))}
                    onRemove={(n) => quiet(() => api(`${key}/sub_issues/${n}`, { method: "DELETE" }))}
                  />
                  <Reactions groups={issue.reactions} path={`${key}/reactions`} canReact={issue.viewer_can_comment} onChange={() => void mutate()} />
                </>
              }
            />
            {items.map((item) =>
              item.type === "comment" ? (
                <CommentBox
                  key={item.id}
                  anchor={`issuecomment-${item.id}`}
                  repo={full}
                  author={item.author.login}
                  association={item.author_association}
                  isIssueAuthor={item.author.login === issue.author.login}
                  verb="commented"
                  createdAt={item.created_at}
                  body={item.body}
                  bodyHtml={item.body_html}
                  editCount={item.edit_count}
                  editsPath={`${repoPath}/issues/comments/${item.id}/edits`}
                  canEdit={item.viewer_can_edit}
                  onSave={(body) => api(`${repoPath}/issues/comments/${item.id}`, { method: "PATCH", body: { body } }).then(() => void refresh())}
                  onDelete={item.viewer_can_delete ? () => api(`${repoPath}/issues/comments/${item.id}`, { method: "DELETE" }).then(() => void refresh()) : undefined}
                  onQuote={issue.viewer_can_comment ? (text) => editor.current?.insert(quote(text)) : undefined}
                  footer={<Reactions groups={item.reactions} path={`${repoPath}/issues/comments/${item.id}/reactions`} canReact={issue.viewer_can_comment} onChange={() => void timeline.mutate()} />}
                />
              ) : (
                <EventRow key={item.id} event={item} base={base} />
              ),
            )}
            <div className="h-4" />
          </div>

          <div className="border-t-2 border-line-muted pt-4">
            {issue.locked && (
              <Flash variant="warning" className="mb-4">
                <span className="flex items-center gap-2">
                  <LockIcon size={16} />
                  This conversation has been locked{issue.lock_reason ? ` as ${issue.lock_reason}` : ""} and limited to collaborators.
                </span>
              </Flash>
            )}
            {user && issue.viewer_can_comment ? (
              <div>
                <div className="flex min-w-0 flex-col gap-2">
                  <h2 className="flex items-center gap-2 text-base font-semibold">
                    <Avatar login={user.login} size={24} />
                    Add a comment
                  </h2>
                  <MarkdownEditor handle={editor} value={comment} onChange={setComment} repo={full} onSubmit={() => hasText && postComment()} />
                  <div className="flex flex-wrap justify-end gap-2">
                    {issue.viewer_can_close &&
                      (issue.state === "open" ? (
                        <div className="flex">
                          <button type="button" disabled={busy} onClick={() => postComment({ state: "closed", state_reason: closeReason })} className={cn(buttonClass("default"), "rounded-r-none")}>
                            {chosen.icon}
                            {hasText ? "Close with comment" : "Close issue"}
                          </button>
                          <ActionMenu
                            width={280}
                            trigger={
                              <button type="button" aria-label="More options" className={cn(buttonClass("default"), "-ml-px rounded-l-none px-2")}>
                                <TriangleDownIcon size={16} />
                              </button>
                            }
                          >
                            {CLOSE_REASONS.map((r) => (
                              <ActionItem key={r.reason} leading={r.icon} onSelect={() => setCloseReason(r.reason)}>
                                <span className="flex-1">
                                  <span className="block">{r.label}</span>
                                  <span className="block text-xs text-fg-muted">{r.hint}</span>
                                </span>
                                {r.reason === closeReason && <CheckIcon size={16} />}
                              </ActionItem>
                            ))}
                          </ActionMenu>
                        </div>
                      ) : (
                        <Button disabled={busy} onClick={() => postComment({ state: "open" })} leading={<IssueReopenedIcon size={16} className="text-fg-success" />}>
                          {hasText ? "Reopen with comment" : "Reopen issue"}
                        </Button>
                      ))}
                    <Button variant="primary" disabled={busy || !hasText} onClick={() => postComment()}>
                      Comment
                    </Button>
                  </div>
                </div>
              </div>
            ) : user ? null : (
              <div className="rounded-md border border-line bg-canvas-muted p-4 text-center text-sm">
                <Link href={`/login?return_to=${encodeURIComponent(`${base}/${number}`)}`} className={buttonClass("primary")}>
                  Sign in
                </Link>
                <span className="ml-2 text-fg-muted">to join this conversation.</span>
              </div>
            )}
          </div>
        </div>

        <aside className="w-full shrink-0 lg:w-[296px]">
          <MetadataSections
            repo={full}
            value={meta}
            canEdit={issue.viewer_can_triage}
            viewer={user?.login}
            onChange={(change) => patchIssue(metadataBody(change, milestones.data))}
            beforeMilestone={<ProjectsSection repo={full} number={issue.number} canEdit={issue.viewer_can_triage} />}
          />
          <SidebarSection title="Relationships">
            {issue.parent ? (
              <Link href={`${base}/${issue.parent.number}`} className="flex items-start gap-2 text-sm hover:text-fg-accent">
                <span className="flex h-5 items-center">
                  <IssueStateIcon issue={issue.parent} />
                </span>
                <span className="min-w-0">
                  <span className="block text-xs text-fg-muted">Parent issue</span>
                  <span className="font-semibold break-words">{issue.parent.title}</span> <span className="text-fg-muted">#{issue.parent.number}</span>
                </span>
              </Link>
            ) : (
              <p className="text-fg-muted">None yet</p>
            )}
          </SidebarSection>
          {user && (
            <SubscribeSection
              subscribed={issue.viewer_subscribed}
              busy={busy}
              onToggle={() => quiet(() => api(`${key}/subscription`, { method: "PUT", body: { subscribed: !issue.viewer_subscribed } }))}
            />
          )}
          <ParticipantsSection participants={participants} />
          {issue.viewer_can_triage && (
            <SidebarSection title="Issue actions">
              <ul className="-mx-2">
                <li>
                  {issue.locked ? (
                    <button type="button" className={sideAction} onClick={() => quiet(() => api(`${key}/lock`, { method: "DELETE" }))}>
                      <UnlockIcon size={16} className="text-fg-muted" />
                      Unlock conversation
                    </button>
                  ) : (
                    <button
                      type="button"
                      className={sideAction}
                      onClick={() => {
                        setLockReason("");
                        setModalError(null);
                        setModal("lock");
                      }}
                    >
                      <LockIcon size={16} className="text-fg-muted" />
                      Lock conversation
                    </button>
                  )}
                </li>
                {canWrite && (
                  <>
                    <li>
                      <button type="button" className={sideAction} onClick={() => quiet(() => api(`${key}/pin`, { method: issue.pinned ? "DELETE" : "PUT" }))}>
                        <PinIcon size={16} className="text-fg-muted" />
                        {issue.pinned ? "Unpin issue" : "Pin issue"}
                      </button>
                    </li>
                    <li>
                      <button
                        type="button"
                        className={sideAction}
                        onClick={() => {
                          setDestination("");
                          setModalError(null);
                          setModal("transfer");
                        }}
                      >
                        <ArrowRightIcon size={16} className="text-fg-muted" />
                        Transfer issue
                      </button>
                    </li>
                  </>
                )}
                {isAdmin && (
                  <li>
                    <button
                      type="button"
                      className={cn(sideAction, "text-fg-danger")}
                      onClick={() => {
                        setModalError(null);
                        setModal("delete");
                      }}
                    >
                      <TrashIcon size={16} />
                      Delete issue
                    </button>
                  </li>
                )}
              </ul>
            </SidebarSection>
          )}
        </aside>
      </div>

      <Dialog
        open={modal === "lock"}
        onClose={() => setModal(null)}
        title="Lock conversation on this issue"
        footer={
          <>
            <Button onClick={() => setModal(null)}>Cancel</Button>
            <Button variant="primary" disabled={busy} onClick={() => inModal(() => api(`${key}/lock`, { method: "PUT", body: { lock_reason: lockReason || null } }))}>
              Lock conversation on this issue
            </Button>
          </>
        }
      >
        {modalError && <Flash className="mb-3">{modalError}</Flash>}
        <ul className="mb-4 list-disc space-y-1 pl-5 text-sm">
          <li>Other users can't add new comments to this issue.</li>
          <li>You and other collaborators with access to this repository can still leave comments that others can see.</li>
          <li>You can always unlock this issue again in the future.</li>
        </ul>
        <Field label="Reason for locking" htmlFor="lock-reason" caption="Optionally, choose a reason for locking that others can see.">
          <select id="lock-reason" value={lockReason} onChange={(e) => setLockReason(e.target.value)} className={inputClass}>
            <option value="">Choose a reason</option>
            {LOCK_REASONS.map((r) => (
              <option key={r} value={r}>
                {r[0].toUpperCase() + r.slice(1)}
              </option>
            ))}
          </select>
        </Field>
      </Dialog>

      <Dialog
        open={modal === "transfer"}
        onClose={() => setModal(null)}
        title="Transfer this issue"
        footer={
          <>
            <Button onClick={() => setModal(null)}>Cancel</Button>
            <Button
              variant="primary"
              disabled={busy || !destination.includes("/")}
              onClick={() =>
                inModal(
                  async () => {
                    const moved = await api<{ owner: string; repo: string; number: number }>(`${key}/transfer`, { method: "POST", body: { repo: destination.trim() } });
                    navigate(`/${moved.owner}/${moved.repo}/issues/${moved.number}`);
                  },
                  () => {},
                )
              }
            >
              Transfer issue
            </Button>
          </>
        }
      >
        {modalError && <Flash className="mb-3">{modalError}</Flash>}
        <Field label="Destination repository" htmlFor="transfer-to" caption="You need write access to it. Labels are kept where the destination has one of the same name; the milestone is removed.">
          <TextInput id="transfer-to" autoFocus placeholder="owner/repository" autoCapitalize="off" value={destination} onChange={(e) => setDestination(e.target.value)} />
        </Field>
      </Dialog>

      <Dialog
        open={modal === "delete"}
        onClose={() => setModal(null)}
        title="Delete issue?"
        footer={
          <>
            <Button onClick={() => setModal(null)}>Cancel</Button>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() =>
                inModal(
                  () => api(key, { method: "DELETE" }),
                  () => {
                    mutateAny(repoPath);
                    navigate(base);
                  },
                )
              }
            >
              Delete
            </Button>
          </>
        }
      >
        {modalError && <Flash className="mb-3">{modalError}</Flash>}
        <ul className="list-disc space-y-1 pl-5 text-sm">
          <li>This cannot be undone.</li>
          <li>Only administrators can delete issues.</li>
          <li>Deletion removes the issue, its comments and its history from this repository.</li>
        </ul>
      </Dialog>
    </Container>
  );
}
