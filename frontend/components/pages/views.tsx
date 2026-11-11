"use client";

import { ArrowLeftIcon, BookmarkIcon, KebabHorizontalIcon, LockIcon, PeopleIcon } from "@primer/octicons-react";
import { useSearchParams } from "next/navigation";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { PageError } from "@/components/pages/repos";
import { IssuesLayout, IssuesNav } from "@/components/shell/issues-layout";
import { ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, inputClass, TextInput } from "@/components/ui/form";
import { useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, hasRole, type SavedView } from "@/lib/api";
import { DEFAULT_QUERY, searchHref } from "@/lib/query";
import { Link, navigate, Redirect } from "@/lib/router";
import { useRepo, useSession } from "@/lib/session";

export const viewsKey = (full: string) => `/repos/${full}/views`;

function ViewList({
  title,
  views,
  base,
  empty,
  onDelete,
}: {
  title: string;
  views: SavedView[];
  base: string;
  empty: React.ReactNode;
  onDelete: (v: SavedView) => void;
}) {
  return (
    <section className="mb-6">
      <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold">
        {title}
        <span className="inline-flex h-5 items-center rounded-full bg-neutral-muted px-[7px] text-xs">{views.length}</span>
      </h2>
      <div className="rounded-md border border-line">
        {views.length === 0 ? (
          <div className="px-4 py-8 text-center">
            <BookmarkIcon size={24} className="mx-auto text-fg-muted" />
            <h3 className="mt-2 text-xl font-semibold">No {title.toLowerCase()} views yet</h3>
            <p className="mt-1 text-sm text-fg-muted">{empty}</p>
          </div>
        ) : (
          <ul>
            {views.map((v) => (
              <li key={v.id} className="flex items-start gap-2 border-b border-line-muted px-4 pt-2 pb-[10px] last:border-b-0">
                <span className="flex h-6 items-center text-fg-muted">
                  <BookmarkIcon size={16} />
                </span>
                <div className="min-w-0 flex-1">
                  <h3 className="text-base leading-6 font-medium break-words">
                    <Link href={searchHref(`${base}/issues`, v.query)} className="hover:text-fg-accent hover:underline">
                      {v.name}
                    </Link>
                  </h3>
                  {v.description && <p className="text-sm break-words text-fg-muted">{v.description}</p>}
                  <p className="mt-1 font-mono text-xs leading-[18px] break-words text-fg-muted">{v.query}</p>
                </div>
                {v.viewer_can_edit && (
                  <ActionMenu
                    width={160}
                    trigger={
                      <button type="button" aria-label={`Actions for view ${v.name}`} className="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
                        <KebabHorizontalIcon size={16} />
                      </button>
                    }
                  >
                    <ActionItem onSelect={() => navigate(`${base}/issues/views/${v.id}/edit`)}>Edit</ActionItem>
                    <ActionItem danger onSelect={() => onDelete(v)}>
                      Delete
                    </ActionItem>
                  </ActionMenu>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

/** Saved searches of this repository's issues: the viewer's private ones and the shared ones. */
export function ViewsPage({ owner, repo }: { owner: string; repo: string }) {
  useTitle(`Views · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const base = `/${full}`;
  const { user } = useSession();
  const { data: repoData } = useRepo(owner, repo);
  const canShare = hasRole(repoData, "write");
  const { data, error, mutate } = useSWR<SavedView[], ApiError>(viewsKey(full), api);
  const [deleting, setDeleting] = useState<SavedView | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  if (error) return <PageError error={error} />;
  const all = data ?? [];
  const create = (shared: boolean) => `${base}/issues/views/new${shared ? "?shared=1" : ""}`;

  async function remove() {
    if (!deleting) return;
    try {
      await api(`${viewsKey(full)}/${deleting.id}`, { method: "DELETE" });
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
      active="views"
      title="Views"
      actions={
        user && (
          <Button variant="primary" href={create(false)}>
            New view
          </Button>
        )
      }
    >
      {failure && (
        <Flash className="mb-4" onDismiss={() => setFailure(null)}>
          {failure}
        </Flash>
      )}
      {user && (
        <ViewList
          title="Private"
          views={all.filter((v) => !v.shared)}
          base={base}
          onDelete={setDeleting}
          empty={
            <>
              <Link href={create(false)} className="text-fg-accent hover:underline">
                Create a private view
              </Link>{" "}
              to track your work.
            </>
          }
        />
      )}
      <ViewList
        title="Shared"
        views={all.filter((v) => v.shared)}
        base={base}
        onDelete={setDeleting}
        empty={
          canShare ? (
            <>
              <Link href={create(true)} className="text-fg-accent hover:underline">
                Create a shared view
              </Link>{" "}
              for everyone in this repository.
            </>
          ) : (
            "Shared views are made by people with write access."
          )
        }
      />
      <Dialog
        open={!!deleting}
        onClose={() => setDeleting(null)}
        title="Delete view"
        footer={
          <>
            <Button onClick={() => setDeleting(null)}>Cancel</Button>
            <Button variant="danger" onClick={remove}>
              Delete
            </Button>
          </>
        }
      >
        Delete the view <strong>{deleting?.name}</strong>? {deleting?.shared ? "It disappears for everyone in this repository." : "Only you can see it."} No issues are changed.
      </Dialog>
    </IssuesLayout>
  );
}

/** New view, or edit when `id` is given. `?q=` prefills the query and `?shared=1` the visibility. */
export function ViewFormPage({ owner, repo, id }: { owner: string; repo: string; id?: string }) {
  useTitle(`${id ? "Edit" : "New"} view · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const { user } = useSession();
  const existing = useSWR<SavedView, ApiError>(id ? `${viewsKey(full)}/${id}` : null, api);
  if (user === undefined) return null;
  if (user === null) return <Redirect to={`/login?return_to=${encodeURIComponent(`/${full}/issues/views`)}`} />;
  if (existing.error) return <PageError error={existing.error} />;
  if (id && !existing.data) return null;
  return <ViewForm key={existing.data?.id ?? "new"} owner={owner} repo={repo} existing={existing.data} />;
}

function ViewForm({ owner, repo, existing }: { owner: string; repo: string; existing?: SavedView }) {
  const full = `${owner}/${repo}`;
  const back = `/${full}/issues/views`;
  const search = useSearchParams();
  const { data: repoData } = useRepo(owner, repo);
  const canShare = hasRole(repoData, "write");
  const [name, setName] = useState(existing?.name ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [query, setQuery] = useState(existing?.query ?? search.get("q") ?? DEFAULT_QUERY);
  const [shared, setShared] = useState(existing?.shared ?? search.get("shared") === "1");
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    const body = { name, description, query, shared: shared && canShare };
    try {
      if (existing) await api(`${viewsKey(full)}/${existing.id}`, { method: "PATCH", body });
      else await api(viewsKey(full), { method: "POST", body });
      navigate(back);
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  return (
    <div className="lg:flex">
      <IssuesNav owner={owner} repo={repo} active="views" />
      <main className="min-w-0 flex-1 px-4 pt-4 pb-6 md:px-6">
        <Link href={back} className="inline-flex items-center gap-2 text-sm text-fg-muted hover:text-fg-accent">
          <ArrowLeftIcon size={16} />
          Back to views
        </Link>
        <h1 className="mt-2 text-xl leading-[32.5px] font-semibold">{existing ? "Edit view" : "Create view"}</h1>
        <form onSubmit={submit} className="mt-2 flex flex-col gap-4">
          {error && error.errors.length === 0 && <Flash>{error.message}</Flash>}
          <div className="flex flex-col gap-4 md:flex-row md:items-start">
            <div className="min-w-0 flex-1">
              <Field label="Title" htmlFor="view-name" required error={error?.field("name")}>
                <TextInput id="view-name" autoFocus maxLength={100} placeholder="Untitled view" value={name} onChange={(e) => setName(e.target.value)} />
              </Field>
            </div>
            <Field label="Visibility" htmlFor="view-shared">
              <span className="relative block">
                <span className="pointer-events-none absolute top-2 left-3 text-fg-muted">{shared ? <PeopleIcon size={16} /> : <LockIcon size={16} />}</span>
                <select id="view-shared" className={`${inputClass} w-auto pr-8 pl-9`} value={shared ? "shared" : "private"} disabled={!canShare} onChange={(e) => setShared(e.target.value === "shared")}>
                  <option value="private">Private</option>
                  <option value="shared">Shared</option>
                </select>
              </span>
            </Field>
          </div>
          <Field label="Description" htmlFor="view-description" error={error?.field("description")}>
            <TextInput id="view-description" maxLength={500} placeholder="Add a description" value={description} onChange={(e) => setDescription(e.target.value)} />
          </Field>
          <Field label="Query" htmlFor="view-query" required error={error?.field("query")} caption="The same syntax as the issue search box.">
            <TextInput id="view-query" spellCheck={false} className="font-mono" value={query} onChange={(e) => setQuery(e.target.value)} />
          </Field>
          <div className="flex items-center justify-end gap-2">
            <Link href={searchHref(`/${full}/issues`, query)} className="mr-auto text-sm text-fg-accent hover:underline">
              Preview results
            </Link>
            <Button href={back}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={busy || !name.trim() || !query.trim()}>
              Save
            </Button>
          </div>
        </form>
      </main>
    </div>
  );
}
