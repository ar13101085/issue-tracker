"use client";

import { IssueOpenedIcon, LockIcon, RepoIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { Blankslate } from "@/components/shell/blankslate";
import { Avatar } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Field, Flash, TextInput } from "@/components/ui/form";
import { Label, useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, type Repo, type User } from "@/lib/api";
import { Link, navigate, Redirect } from "@/lib/router";
import { useSession } from "@/lib/session";
import { cn } from "@/lib/utils";

export function Container({ children, narrow }: { children: React.ReactNode; narrow?: boolean }) {
  return <main className={cn("mx-auto w-full px-4 py-6 md:px-6 lg:px-8", narrow ? "max-w-[1012px]" : "max-w-[1280px]")}>{children}</main>;
}

export function PageError({ error }: { error: ApiError }) {
  if (error.status === 404) return <NotFound />;
  return (
    <Container>
      <Flash>{error.message}</Flash>
    </Container>
  );
}

export function NotFound() {
  useTitle("Page not found · Issue Tracker");
  return (
    <Container>
      <Blankslate title="404" description="This is not the web page you are looking for." />
    </Container>
  );
}

function RepoList({ repos, showOwner }: { repos: Repo[]; showOwner?: boolean }) {
  return (
    <ul className="border-t border-line-muted">
      {repos.map((r) => (
        <li key={`${r.owner}/${r.name}`} className="flex items-start gap-4 border-b border-line-muted py-6">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <Link href={`/${r.owner}/${r.name}`} className="text-xl font-semibold break-all text-fg-accent hover:underline">
                {showOwner ? `${r.owner}/${r.name}` : r.name}
              </Link>
              <Label>{r.private ? "Private" : "Public"}</Label>
            </div>
            {r.description && <p className="mt-1 max-w-3xl text-fg-muted">{r.description}</p>}
          </div>
          <Link href={`/${r.owner}/${r.name}/issues`} className="flex shrink-0 items-center gap-1 pt-1.5 text-xs text-fg-muted hover:text-fg-accent">
            <IssueOpenedIcon size={16} />
            {r.open_issues_count}
          </Link>
        </li>
      ))}
    </ul>
  );
}

export function HomePage() {
  useTitle("Issue Tracker");
  const { user } = useSession();
  const { data, error } = useSWR<Repo[], ApiError>(user ? "/user/repos" : null, api);
  if (user === undefined) return null;
  if (user === null) {
    return (
      <Container>
        <div className="flex flex-col items-center px-6 py-16 text-center">
          <IssueOpenedIcon size={48} className="text-fg-muted" />
          <h1 className="mt-4 text-2xl font-semibold">Track issues for your projects</h1>
          <p className="mt-1 max-w-md text-base text-fg-muted">Sign in to see your repositories, open issues and collaborate with your team.</p>
          <div className="mt-6 flex gap-2">
            <Button variant="primary" href="/login">
              Sign in
            </Button>
            <Button href="/signup">Sign up</Button>
          </div>
        </div>
      </Container>
    );
  }
  if (error) return <PageError error={error} />;
  return (
    <Container narrow>
      <div className="flex items-center justify-between gap-2 pb-4">
        <h1 className="text-2xl font-normal">Repositories</h1>
        <Button variant="primary" href="/new" leading={<RepoIcon size={16} />}>
          New
        </Button>
      </div>
      {data && data.length === 0 && (
        <div className="rounded-md border border-line">
          <Blankslate icon={<RepoIcon size={24} />} title="You don't have any repositories yet" description="Create one to start tracking issues." />
        </div>
      )}
      {data && data.length > 0 && <RepoList repos={data} showOwner />}
    </Container>
  );
}

export function OwnerPage({ owner }: { owner: string }) {
  useTitle(`${owner} · Issue Tracker`);
  const profile = useSWR<User, ApiError>(`/users/${owner}`, api);
  const repos = useSWR<Repo[], ApiError>(`/users/${owner}/repos`, api);
  if (profile.error) return <PageError error={profile.error} />;
  if (!profile.data) return null;
  return (
    <Container narrow>
      <div className="flex items-center gap-4 pb-6">
        <Avatar login={profile.data.login} size={64} />
        <div className="min-w-0">
          <h1 className="truncate text-2xl font-semibold">{profile.data.name ?? profile.data.login}</h1>
          <p className="text-xl font-light text-fg-muted">{profile.data.login}</p>
        </div>
      </div>
      <h2 className="pb-2 text-base font-semibold">Repositories</h2>
      {repos.data && repos.data.length === 0 && (
        <div className="rounded-md border border-line">
          <Blankslate icon={<RepoIcon size={24} />} title={`${profile.data.login} doesn't have any public repositories yet.`} />
        </div>
      )}
      {repos.data && repos.data.length > 0 && <RepoList repos={repos.data} />}
    </Container>
  );
}

export function NewRepoPage() {
  useTitle("New repository · Issue Tracker");
  const { user } = useSession();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [isPrivate, setPrivate] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  if (user === undefined) return null;
  if (user === null) return <Redirect to="/login?return_to=/new" />;

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const repo = await api<Repo>("/user/repos", { method: "POST", body: { name, description, private: isPrivate } });
      navigate(`/${repo.owner}/${repo.name}/issues`);
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  const choice = "flex cursor-pointer items-start gap-2 py-2";
  return (
    <main className="mx-auto w-full max-w-[768px] px-4 py-6 md:py-10">
      <h1 className="text-2xl font-semibold">Create a new repository</h1>
      <p className="mt-1 border-b border-line-muted pb-4 text-fg-muted">A repository holds the issues of one project.</p>
      <form onSubmit={submit} className="mt-4 flex flex-col gap-4">
        {error && error.errors.length === 0 && <Flash>{error.message}</Flash>}
        <div className="flex flex-wrap items-end gap-2">
          <div className="flex flex-col gap-1">
            <span className="text-sm font-semibold">Owner</span>
            <span className="flex h-8 items-center gap-2 rounded-md border border-line bg-control px-3 text-sm font-medium">
              <Avatar login={user.login} size={16} />
              {user.login}
            </span>
          </div>
          <span className="pb-1 text-xl text-fg-muted">/</span>
          <div className="min-w-[200px] flex-1">
            <Field label="Repository name" htmlFor="repo-name" required error={error?.field("name")}>
              <TextInput id="repo-name" autoFocus required maxLength={100} aria-invalid={!!error?.field("name")} value={name} onChange={(e) => setName(e.target.value)} />
            </Field>
          </div>
        </div>
        <Field label="Description" htmlFor="repo-description" error={error?.field("description")} caption={`${description.length} / 350 characters`}>
          <TextInput id="repo-description" maxLength={350} value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <fieldset className="border-y border-line-muted py-2">
          <legend className="sr-only">Visibility</legend>
          <label className={choice}>
            <input type="radio" name="visibility" className="mt-1" checked={!isPrivate} onChange={() => setPrivate(false)} />
            <RepoIcon size={24} className="mt-0.5 text-fg-muted" />
            <span>
              <span className="block font-semibold">Public</span>
              <span className="text-xs text-fg-muted">Anyone can see this repository. Signed-in users can open issues.</span>
            </span>
          </label>
          <label className={choice}>
            <input type="radio" name="visibility" className="mt-1" checked={isPrivate} onChange={() => setPrivate(true)} />
            <LockIcon size={24} className="mt-0.5 text-fg-muted" />
            <span>
              <span className="block font-semibold">Private</span>
              <span className="text-xs text-fg-muted">You choose who can see and work in this repository.</span>
            </span>
          </label>
        </fieldset>
        <div className="flex justify-end">
          <Button type="submit" variant="primary" disabled={busy || !name.trim()}>
            {busy ? "Creating…" : "Create repository"}
          </Button>
        </div>
      </form>
    </main>
  );
}
