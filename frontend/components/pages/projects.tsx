"use client";

import { CheckIcon, TableIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { Container, PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, TextInput } from "@/components/ui/form";
import { RelativeTime, useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, hasRole, type Project, type ProjectSummary } from "@/lib/api";
import { Link, navigate } from "@/lib/router";
import { useRepo } from "@/lib/session";
import { cn } from "@/lib/utils";

export function ProjectsPage({ owner, repo }: { owner: string; repo: string }) {
  useTitle(`Projects · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const { data: repoData } = useRepo(owner, repo);
  const canWrite = hasRole(repoData, "write");
  const { data, error } = useSWR<ProjectSummary[], ApiError>(`/repos/${full}/projects`, api);
  const [tab, setTab] = useState<"open" | "closed">("open");
  const [creating, setCreating] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [failure, setFailure] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  if (error) return <PageError error={error} />;
  const all = data ?? [];
  const shown = all.filter((p) => p.closed === (tab === "closed"));

  async function create(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setFailure(null);
    try {
      const made = await api<Project>(`/repos/${full}/projects`, { method: "POST", body: { title, description } });
      navigate(`/${full}/projects/${made.number}`);
    } catch (err) {
      setFailure(asApiError(err));
      setBusy(false);
    }
  }

  const tabClass = (on: boolean) => cn("flex cursor-pointer items-center gap-1 text-sm", on ? "font-semibold text-fg" : "text-fg-muted hover:text-fg");
  return (
    <Container>
      <div className="flex items-center justify-between gap-2">
        <h1 className="text-2xl font-normal">Projects</h1>
        {canWrite && (
          <Button variant="primary" onClick={() => setCreating(true)}>
            New project
          </Button>
        )}
      </div>
      <div className="mt-4 rounded-md border border-line">
        <div className="flex min-h-12 items-center gap-4 rounded-t-md border-b border-line bg-canvas-muted px-4">
          <button type="button" className={tabClass(tab === "open")} onClick={() => setTab("open")}>
            <TableIcon size={16} />
            {all.filter((p) => !p.closed).length} Open
          </button>
          <button type="button" className={tabClass(tab === "closed")} onClick={() => setTab("closed")}>
            <CheckIcon size={16} />
            {all.filter((p) => p.closed).length} Closed
          </button>
        </div>
        {data && shown.length === 0 && (
          <Blankslate
            icon={<TableIcon size={24} />}
            title={tab === "open" ? "No open projects" : "No closed projects"}
            description="Projects organise issues into a table or a board with your own fields."
          />
        )}
        <ul className="divide-y divide-line-muted">
          {shown.map((p) => (
            <li key={p.number} className="flex items-start gap-3 p-4">
              <TableIcon size={16} className="mt-1 shrink-0 text-fg-muted" />
              <div className="min-w-0 flex-1">
                <Link href={`/${full}/projects/${p.number}`} className="text-base font-semibold break-words hover:text-fg-accent">
                  {p.title}
                </Link>
                {p.description && <p className="text-sm break-words text-fg-muted">{p.description}</p>}
                <p className="mt-1 text-xs text-fg-muted">
                  #{p.number} · {p.item_count} item{p.item_count === 1 ? "" : "s"} · updated <RelativeTime iso={p.updated_at} />
                </p>
              </div>
            </li>
          ))}
        </ul>
      </div>

      <Dialog open={creating} onClose={() => setCreating(false)} title="New project">
        <form onSubmit={create} className="flex flex-col gap-4">
          {failure && failure.errors.length === 0 && <Flash>{failure.message}</Flash>}
          <Field label="Project name" htmlFor="project-title" required error={failure?.field("title")}>
            <TextInput id="project-title" autoFocus maxLength={255} value={title} onChange={(e) => setTitle(e.target.value)} />
          </Field>
          <Field label="Description" htmlFor="project-description">
            <TextInput id="project-description" value={description} onChange={(e) => setDescription(e.target.value)} />
          </Field>
          <div className="flex justify-end gap-2">
            <Button onClick={() => setCreating(false)}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={busy || !title.trim()}>
              Create project
            </Button>
          </div>
        </form>
      </Dialog>
    </Container>
  );
}
