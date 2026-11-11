"use client";

import { AlertFillIcon, MilestoneIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { Container, PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { IssuesLayout, ListBox, ListTabs } from "@/components/shell/issues-layout";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, Textarea, TextInput } from "@/components/ui/form";
import { RelativeTime, useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, hasRole, type Milestone } from "@/lib/api";
import { searchHref, setToken } from "@/lib/query";
import { Link, navigate } from "@/lib/router";
import { useRepo } from "@/lib/session";
import { cn } from "@/lib/utils";

function dueText(m: Milestone): { text: string; late: boolean } {
  if (m.state === "closed" && m.closed_at) return { text: `Closed ${new Date(m.closed_at).toLocaleDateString("en-US", { dateStyle: "medium" })}`, late: false };
  if (!m.due_on) return { text: "No due date", late: false };
  const due = new Date(m.due_on);
  const late = due.getTime() < Date.now();
  const date = due.toLocaleDateString("en-US", { dateStyle: "long" });
  return { text: late ? `Overdue by ${Math.max(1, Math.round((Date.now() - due.getTime()) / 86_400_000))} day(s)` : `Due by ${date}`, late };
}

export function Progress({ open, closed, className, openHref, closedHref }: { open: number; closed: number; className?: string; openHref?: string; closedHref?: string }) {
  const total = open + closed;
  const percent = total === 0 ? 0 : Math.round((closed / total) * 100);
  const count = (n: number, word: string, href?: string) => {
    const body = (
      <>
        <span className="font-semibold text-fg">{n}</span> {word}
      </>
    );
    return href ? (
      <Link href={href} className="hover:text-fg-accent">
        {body}
      </Link>
    ) : (
      <span>{body}</span>
    );
  };
  return (
    <div className={className}>
      <div className="h-[5px] overflow-hidden rounded-[3px] bg-[var(--borderColor-default)]" role="progressbar" aria-valuenow={percent} aria-valuemin={0} aria-valuemax={100}>
        <div className="h-full bg-success-emphasis" style={{ width: `${percent}%` }} />
      </div>
      <p className="mt-2 flex gap-4 text-sm text-fg-muted">
        <span>
          <span className="font-semibold text-fg">{percent}%</span> complete
        </span>
        {count(open, "open", openHref)}
        {count(closed, "closed", closedHref)}
      </p>
    </div>
  );
}

export function MilestonesPage({ owner, repo }: { owner: string; repo: string }) {
  useTitle(`Milestones · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const base = `/${owner}/${repo}`;
  const { data: repoData } = useRepo(owner, repo);
  const canWrite = hasRole(repoData, "write");
  const { data, error, mutate } = useSWR<Milestone[], ApiError>(`/repos/${full}/milestones?state=all`, api);
  const [tab, setTab] = useState<"open" | "closed">("open");
  const [deleting, setDeleting] = useState<Milestone | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  if (error) return <PageError error={error} />;
  const all = data ?? [];
  const shown = all.filter((m) => m.state === tab);
  const count = (s: string) => all.filter((m) => m.state === s).length;

  async function run(action: () => Promise<unknown>) {
    setFailure(null);
    try {
      await action();
      await mutate();
    } catch (err) {
      setFailure(asApiError(err).summary);
    }
    setDeleting(null);
  }

  return (
    <IssuesLayout
      owner={owner}
      repo={repo}
      active="milestones"
      title="Milestones"
      actions={
        canWrite && (
          <Button variant="primary" href={`${base}/milestones/new`}>
            New milestone
          </Button>
        )
      }
    >
      {failure && (
        <Flash className="mb-4" onDismiss={() => setFailure(null)}>
          {failure}
        </Flash>
      )}
      <ListBox
        header={
          <ListTabs
            tabs={[
              { key: "open", label: "Open", count: count("open") },
              { key: "closed", label: "Closed", count: count("closed") },
            ]}
            value={tab}
            onChange={setTab}
          />
        }
      >
        {data && shown.length === 0 && (
          <Blankslate
            icon={<MilestoneIcon size={24} />}
            title={tab === "open" ? "You haven't created any milestones." : "There are no closed milestones."}
            description="Use milestones to track progress on groups of issues."
          />
        )}
        <ul>
          {shown.map((m) => {
            const due = dueText(m);
            const total = m.open_issues + m.closed_issues;
            const issuesHref = (state: string) => searchHref(`${base}/issues`, setToken(`is:issue state:${state}`, "milestone", m.title));
            return (
              <li key={m.number} className="mx-4 flex flex-col gap-x-1 border-b border-line-muted pb-[10px] last:border-b-0 md:flex-row">
                <div className="min-w-0 flex-1 px-2 pt-2">
                  <h3 className="text-base leading-6 font-medium break-words">
                    <Link href={issuesHref("open")} className="hover:text-fg-accent hover:underline">
                      {m.title}
                    </Link>
                  </h3>
                  {m.description && <p className="mt-1 text-sm break-words whitespace-pre-wrap text-fg-muted">{m.description}</p>}
                  <p className="mt-1 flex flex-wrap items-center gap-x-2 text-xs leading-[18px] text-fg-muted">
                    <span className={cn("flex items-center gap-1", due.late && "text-fg-danger")}>
                      {due.late && <AlertFillIcon size={12} />}
                      {due.text}
                    </span>
                    {total > 0 && (
                      <>
                        <span aria-hidden>•</span>
                        <span>
                          {m.closed_issues}/{total} issues closed
                        </span>
                      </>
                    )}
                    <span aria-hidden>•</span>
                    <span>
                      Last updated <RelativeTime iso={m.updated_at} />
                    </span>
                  </p>
                </div>
                <div className="px-2 pt-[18px] md:mr-1 md:w-[320px] md:shrink-0 md:px-0">
                  <Progress open={m.open_issues} closed={m.closed_issues} openHref={issuesHref("open")} closedHref={issuesHref("closed")} />
                  {canWrite && (
                    <div className="mt-1 flex gap-4 text-xs">
                      <Link href={`${base}/milestones/${m.number}/edit`} className="text-fg-accent hover:underline">
                        Edit
                      </Link>
                      <button
                        type="button"
                        className="cursor-pointer text-fg-accent hover:underline"
                        onClick={() => run(() => api(`/repos/${full}/milestones/${m.number}`, { method: "PATCH", body: { state: m.state === "open" ? "closed" : "open" } }))}
                      >
                        {m.state === "open" ? "Close" : "Reopen"}
                      </button>
                      <button type="button" className="cursor-pointer text-fg-danger hover:underline" onClick={() => setDeleting(m)}>
                        Delete
                      </button>
                    </div>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      </ListBox>
      <Dialog
        open={!!deleting}
        onClose={() => setDeleting(null)}
        title="Delete milestone"
        footer={
          <>
            <Button onClick={() => setDeleting(null)}>Cancel</Button>
            <Button variant="danger" onClick={() => deleting && run(() => api(`/repos/${full}/milestones/${deleting.number}`, { method: "DELETE" }))}>
              Delete this milestone
            </Button>
          </>
        }
      >
        Deleting <strong>{deleting?.title}</strong> cannot be undone. Its issues are kept and simply lose the milestone.
      </Dialog>
    </IssuesLayout>
  );
}

/** New milestone, or edit when `number` is given. */
export function MilestoneFormPage({ owner, repo, number }: { owner: string; repo: string; number?: number }) {
  useTitle(`${number ? "Edit" : "New"} milestone · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const back = `/${owner}/${repo}/milestones`;
  const existing = useSWR<Milestone, ApiError>(number ? `/repos/${full}/milestones/${number}` : null, api);
  if (existing.error) return <PageError error={existing.error} />;
  if (number && !existing.data) return null;
  return <MilestoneForm key={existing.data?.number ?? "new"} full={full} back={back} existing={existing.data} />;
}

function MilestoneForm({ full, back, existing }: { full: string; back: string; existing?: Milestone }) {
  const [title, setTitle] = useState(existing?.title ?? "");
  const [due, setDue] = useState(existing?.due_on ? existing.due_on.slice(0, 10) : "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    const body = { title, description, due_on: due ? `${due}T00:00:00Z` : null };
    try {
      if (existing) await api(`/repos/${full}/milestones/${existing.number}`, { method: "PATCH", body });
      else await api(`/repos/${full}/milestones`, { method: "POST", body });
      navigate(back);
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  return (
    <Container narrow>
      <h1 className="text-2xl font-normal">{existing ? "Edit milestone" : "New milestone"}</h1>
      <p className="mt-1 border-b border-line-muted pb-4 text-sm text-fg-muted">Milestones group issues toward a goal and show how far along it is.</p>
      <form onSubmit={submit} className="mt-4 flex max-w-xl flex-col gap-4">
        {error && error.errors.length === 0 && <Flash>{error.message}</Flash>}
        <Field label="Title" htmlFor="milestone-title" required error={error?.field("title")}>
          <TextInput id="milestone-title" autoFocus required maxLength={255} placeholder="Title" value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label="Due date (optional)" htmlFor="milestone-due">
          <TextInput id="milestone-due" type="date" className="w-48" value={due} onChange={(e) => setDue(e.target.value)} />
        </Field>
        <Field label="Description" htmlFor="milestone-description">
          <Textarea id="milestone-description" rows={6} value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <div className="flex justify-end gap-2 border-t border-line-muted pt-4">
          <Button href={back}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={busy || !title.trim()}>
            {existing ? "Save changes" : "Create milestone"}
          </Button>
        </div>
      </form>
    </Container>
  );
}
