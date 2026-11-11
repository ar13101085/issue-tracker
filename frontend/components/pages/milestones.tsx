"use client";

import { CalendarIcon, CheckIcon, ClockIcon, MilestoneIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { Container, PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
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
  return { text: late ? `Past due by ${Math.max(1, Math.round((Date.now() - due.getTime()) / 86_400_000))} days` : `Due by ${date}`, late };
}

export function Progress({ open, closed, className }: { open: number; closed: number; className?: string }) {
  const total = open + closed;
  const percent = total === 0 ? 0 : Math.round((closed / total) * 100);
  return (
    <div className={className}>
      <div className="h-2 overflow-hidden rounded-full bg-neutral-muted" role="progressbar" aria-valuenow={percent} aria-valuemin={0} aria-valuemax={100}>
        <div className="h-full rounded-full bg-success-emphasis" style={{ width: `${percent}%` }} />
      </div>
      <p className="mt-2 text-xs text-fg-muted">
        <span className="font-semibold text-fg">{percent}%</span> complete <span className="ml-2 font-semibold text-fg">{open}</span> open{" "}
        <span className="ml-2 font-semibold text-fg">{closed}</span> closed
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

  const tabClass = (on: boolean) => cn("flex cursor-pointer items-center gap-1 text-sm", on ? "font-semibold text-fg" : "text-fg-muted hover:text-fg");
  return (
    <Container>
      <div className="flex flex-wrap items-center gap-2">
        <Button href={`${base}/issues`}>Issues</Button>
        <Button href={`${base}/labels`}>Labels</Button>
        {canWrite && (
          <Button variant="primary" className="ml-auto" href={`${base}/milestones/new`}>
            New milestone
          </Button>
        )}
      </div>
      {failure && (
        <Flash className="mt-4" onDismiss={() => setFailure(null)}>
          {failure}
        </Flash>
      )}
      <div className="mt-4 rounded-md border border-line">
        <div className="flex min-h-12 items-center gap-4 rounded-t-md border-b border-line bg-canvas-muted px-4">
          <button type="button" className={tabClass(tab === "open")} onClick={() => setTab("open")}>
            <MilestoneIcon size={16} />
            {count("open")} Open
          </button>
          <button type="button" className={tabClass(tab === "closed")} onClick={() => setTab("closed")}>
            <CheckIcon size={16} />
            {count("closed")} Closed
          </button>
        </div>
        {data && shown.length === 0 && (
          <Blankslate
            icon={<MilestoneIcon size={24} />}
            title={tab === "open" ? "You haven't created any milestones." : "There are no closed milestones."}
            description="Use milestones to track progress on groups of issues."
          />
        )}
        <ul className="divide-y divide-line-muted">
          {shown.map((m) => {
            const due = dueText(m);
            const issuesHref = searchHref(`${base}/issues`, setToken("is:issue state:open", "milestone", m.title));
            return (
              <li key={m.number} className="flex flex-col gap-4 p-4 md:flex-row">
                <div className="min-w-0 md:w-1/2">
                  <h2 className="text-2xl font-normal break-words">
                    <Link href={issuesHref} className="hover:text-fg-accent">
                      {m.title}
                    </Link>
                  </h2>
                  <p className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-fg-muted">
                    <span className={cn("flex items-center gap-1", due.late && "text-fg-danger")}>
                      <CalendarIcon size={16} />
                      {due.text}
                    </span>
                    <span className="flex items-center gap-1">
                      <ClockIcon size={16} />
                      Last updated <RelativeTime iso={m.updated_at} />
                    </span>
                  </p>
                  {m.description && <p className="mt-2 text-sm break-words whitespace-pre-wrap text-fg-muted">{m.description}</p>}
                </div>
                <div className="md:w-1/2">
                  <Progress open={m.open_issues} closed={m.closed_issues} />
                  {canWrite && (
                    <div className="mt-2 flex gap-4 text-sm">
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
      </div>
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
    </Container>
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
