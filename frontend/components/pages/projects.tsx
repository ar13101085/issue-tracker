"use client";

import { SearchIcon, TableIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { IssuesLayout, ListBox, ListTabs } from "@/components/shell/issues-layout";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, inputClass, TextInput } from "@/components/ui/form";
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
  const [filter, setFilter] = useState("");
  const [creating, setCreating] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [failure, setFailure] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  if (error) return <PageError error={error} />;
  const all = data ?? [];
  const needle = filter.trim().toLowerCase();
  const shown = all.filter((p) => p.closed === (tab === "closed") && (!needle || p.title.toLowerCase().includes(needle)));

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

  return (
    <IssuesLayout
      owner={owner}
      repo={repo}
      active="projects"
      title="Projects"
      actions={
        canWrite && (
          <Button variant="primary" onClick={() => setCreating(true)}>
            New project
          </Button>
        )
      }
    >
      <div className="relative pb-4">
        <SearchIcon size={16} className="absolute top-2 left-2 text-fg-muted" />
        <input aria-label="Search by name" placeholder="Search by name..." value={filter} onChange={(e) => setFilter(e.target.value)} className={cn(inputClass, "pl-8")} />
      </div>
      <ListBox
        header={
          <ListTabs
            tabs={[
              { key: "open", label: "Open", count: all.filter((p) => !p.closed).length },
              { key: "closed", label: "Closed", count: all.filter((p) => p.closed).length },
            ]}
            value={tab}
            onChange={setTab}
          />
        }
      >
        {data && shown.length === 0 && (
          <Blankslate
            icon={<TableIcon size={24} />}
            title={needle ? "No matching projects" : tab === "open" ? "No open projects" : "No closed projects"}
            description="Projects organise issues into a table or a board with your own fields."
          />
        )}
        <ul>
          {shown.map((p) => (
            <li key={p.number} className="flex items-start gap-2 border-b border-line-muted px-4 pt-2 pb-[10px] last:border-b-0">
              <span className="flex h-6 items-center">
                <TableIcon size={16} className="shrink-0 text-fg-muted" />
              </span>
              <div className="min-w-0 flex-1">
                <h3 className="text-base leading-6 font-medium break-words">
                  <Link href={`/${full}/projects/${p.number}`} className="hover:text-fg-accent hover:underline">
                    {p.title}
                  </Link>
                </h3>
                {p.description && <p className="text-sm break-words text-fg-muted">{p.description}</p>}
                <p className="mt-1 text-xs leading-[18px] text-fg-muted">
                  #{p.number} • {p.item_count} item{p.item_count === 1 ? "" : "s"} • updated <RelativeTime iso={p.updated_at} />
                </p>
              </div>
            </li>
          ))}
        </ul>
      </ListBox>

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
    </IssuesLayout>
  );
}
