"use client";

import { GearIcon, NoteIcon, PeopleIcon, PlusIcon, TrashIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR, { useSWRConfig } from "swr";
import { NotFound, PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { Avatar } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Field, Flash, inputClass, Textarea, TextInput } from "@/components/ui/form";
import { useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, type Collaborator, hasRole, ISSUE_TYPES, type Repo, type Role, type Template } from "@/lib/api";
import { Link, navigate } from "@/lib/router";
import { useRepo } from "@/lib/session";
import { cn } from "@/lib/utils";
import { SettingsFrame, Subhead } from "@/components/shell/settings-frame";

const ROLES: { value: Role; label: string; hint: string }[] = [
  { value: "read", label: "Read", hint: "Can view and open issues" },
  { value: "triage", label: "Triage", hint: "Can also close, reopen and organize issues" },
  { value: "write", label: "Write", hint: "Can also edit any issue" },
  { value: "admin", label: "Admin", hint: "Full access, including settings" },
];

export function SettingsPage({ owner, repo, section }: { owner: string; repo: string; section?: string }) {
  useTitle(`Settings · ${owner}/${repo}`);
  const { data, error, mutate } = useRepo(owner, repo);
  if (error) return <PageError error={error} />;
  if (!data) return null;
  if (!hasRole(data, "admin") || (section && section !== "access" && section !== "templates")) return <NotFound />;
  const base = `/${owner}/${repo}/settings`;
  return (
    <SettingsFrame
      nav={[
        { href: base, active: !section, icon: <GearIcon size={16} />, label: "General" },
        { href: `${base}/access`, active: section === "access", icon: <PeopleIcon size={16} />, label: "Collaborators" },
        { href: `${base}/templates`, active: section === "templates", icon: <NoteIcon size={16} />, label: "Issue templates" },
      ]}
    >
      {section === "access" ? <Access repo={data} /> : section === "templates" ? <Templates repo={data} /> : <General repo={data} onChange={(r) => mutate(r, { revalidate: false })} />}
    </SettingsFrame>
  );
}

function General({ repo, onChange }: { repo: Repo; onChange: (r: Repo) => void }) {
  const path = `/repos/${repo.owner}/${repo.name}`;
  const [name, setName] = useState(repo.name);
  const [description, setDescription] = useState(repo.description);
  const [error, setError] = useState<ApiError | null>(null);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState("");
  const [deleting, setDeleting] = useState(false);

  async function patch(body: Record<string, unknown>) {
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      const next = await api<Repo>(path, { method: "PATCH", body });
      if (next.name !== repo.name) navigate(`/${next.owner}/${next.name}/settings`, { replace: true });
      else onChange(next);
      setSaved(true);
    } catch (err) {
      setError(asApiError(err));
    } finally {
      setBusy(false);
    }
  }

  async function remove(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api(path, { method: "DELETE" });
      navigate("/");
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  const full = `${repo.owner}/${repo.name}`;
  const row = "flex flex-col gap-2 p-4 md:flex-row md:items-center md:justify-between";
  return (
    <>
      <Subhead>General</Subhead>
      {error && error.errors.length === 0 && <Flash className="mb-4">{error.message}</Flash>}
      {saved && (
        <Flash variant="success" className="mb-4" onDismiss={() => setSaved(false)}>
          Settings saved.
        </Flash>
      )}
      <form
        onSubmit={(e) => {
          e.preventDefault();
          patch({ name, description });
        }}
        className="flex max-w-xl flex-col gap-4"
      >
        <Field label="Repository name" htmlFor="rename" required error={error?.field("name")}>
          <TextInput id="rename" required maxLength={100} aria-invalid={!!error?.field("name")} value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Description" htmlFor="description" error={error?.field("description")}>
          <TextInput id="description" maxLength={350} value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <div>
          <Button type="submit" disabled={busy || !name.trim() || (name === repo.name && description === repo.description)}>
            Save changes
          </Button>
        </div>
      </form>

      <div className="mt-10">
        <Subhead danger>Danger Zone</Subhead>
        <div className="divide-y divide-line-muted rounded-md border border-[var(--borderColor-danger-emphasis)]">
          <div className={row}>
            <div>
              <p className="font-semibold">Change repository visibility</p>
              <p className="text-sm">This repository is currently {repo.private ? "private" : "public"}.</p>
            </div>
            <Button variant="danger" disabled={busy} onClick={() => patch({ private: !repo.private })}>
              Change to {repo.private ? "public" : "private"}
            </Button>
          </div>
          <div className={row}>
            <div>
              <p className="font-semibold">Delete this repository</p>
              <p className="text-sm">Once you delete a repository, there is no going back. All of its issues are deleted with it.</p>
            </div>
            {!deleting && (
              <Button variant="danger" onClick={() => setDeleting(true)}>
                Delete this repository
              </Button>
            )}
          </div>
          {deleting && (
            <form onSubmit={remove} className="flex flex-col gap-2 p-4">
              <label htmlFor="confirm-delete" className="text-sm font-semibold">
                To confirm, type &quot;{full}&quot; in the box below
              </label>
              <TextInput id="confirm-delete" autoFocus autoComplete="off" value={confirm} onChange={(e) => setConfirm(e.target.value)} className="max-w-xl border-[var(--borderColor-danger-emphasis)]" />
              <div className="flex gap-2">
                <Button type="submit" variant="danger" disabled={busy || confirm !== full} leading={<TrashIcon size={16} />}>
                  Delete this repository
                </Button>
                <Button
                  onClick={() => {
                    setDeleting(false);
                    setConfirm("");
                  }}
                >
                  Cancel
                </Button>
              </div>
            </form>
          )}
        </div>
      </div>
    </>
  );
}

function RoleSelect({ value, onChange, label, disabled }: { value: Role; onChange: (r: Role) => void; label: string; disabled?: boolean }) {
  return (
    <select aria-label={label} disabled={disabled} value={value} onChange={(e) => onChange(e.target.value as Role)} className={cn(inputClass, "w-auto pr-8")}>
      {ROLES.map((r) => (
        <option key={r.value} value={r.value}>
          {r.label}
        </option>
      ))}
    </select>
  );
}

function Access({ repo }: { repo: Repo }) {
  const path = `/repos/${repo.owner}/${repo.name}/collaborators`;
  const { data, error } = useSWR<Collaborator[], ApiError>(path, api);
  const { mutate } = useSWRConfig();
  const [login, setLogin] = useState("");
  const [role, setRole] = useState<Role>("write");
  const [failure, setFailure] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setFailure(null);
    try {
      await action();
      await mutate(path);
    } catch (err) {
      setFailure(asApiError(err).summary);
    } finally {
      setBusy(false);
    }
  }

  function add(e: FormEvent) {
    e.preventDefault();
    run(async () => {
      await api(`${path}/${encodeURIComponent(login.trim())}`, { method: "PUT", body: { role } });
      setLogin("");
    });
  }

  return (
    <>
      <Subhead>Collaborators</Subhead>
      <p className="mb-4 text-sm text-fg-muted">
        This repository is {repo.private ? "private: only the people below can see it" : "public: anyone can see it, and signed-in users can open issues"}. Roles
        build on each other: {ROLES.map((r) => `${r.label} (${r.hint.toLowerCase()})`).join(", ")}.
      </p>
      {(failure || error) && (
        <Flash className="mb-4" onDismiss={failure ? () => setFailure(null) : undefined}>
          {failure ?? error?.message}
        </Flash>
      )}
      <form onSubmit={add} className="mb-4 flex flex-wrap items-end gap-2">
        <div className="min-w-[200px] flex-1">
          <Field label="Add people" htmlFor="collab-login">
            <TextInput id="collab-login" placeholder="Username" autoCapitalize="off" autoComplete="off" value={login} onChange={(e) => setLogin(e.target.value)} />
          </Field>
        </div>
        <RoleSelect label="Role for the new collaborator" value={role} onChange={setRole} />
        <Button type="submit" variant="primary" disabled={busy || !login.trim()}>
          Add collaborator
        </Button>
      </form>
      <div className="rounded-md border border-line">
        <div className="flex min-h-[50px] items-center rounded-t-md border-b border-line bg-canvas-muted px-4 py-2 text-sm font-medium">
          {data ? `${data.length} collaborator${data.length === 1 ? "" : "s"}` : "Collaborators"}
        </div>
        {data && data.length === 0 && <Blankslate icon={<PeopleIcon size={24} />} title="You haven't added any collaborators yet" />}
        <ul className="divide-y divide-line-muted">
          {data?.map((c) => (
            <li key={c.login} className="flex min-h-[72px] flex-wrap items-center gap-4 p-4">
              <Avatar login={c.login} size={32} />
              <div className="min-w-0 flex-1">
                <Link href={`/${c.login}`} className="block truncate text-sm font-semibold text-fg-accent hover:underline">
                  {c.login}
                </Link>
                <span className="block text-xs leading-[18px] text-fg-muted">Collaborator</span>
              </div>
              <RoleSelect
                label={`Role of ${c.login}`}
                disabled={busy}
                value={c.role}
                onChange={(r) => run(() => api(`${path}/${encodeURIComponent(c.login)}`, { method: "PUT", body: { role: r } }))}
              />
              <Button variant="danger" className="px-2" disabled={busy} aria-label={`Remove ${c.login}`} title="Remove" onClick={() => run(() => api(`${path}/${encodeURIComponent(c.login)}`, { method: "DELETE" }))}>
                <TrashIcon size={16} />
              </Button>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}

const BLANK: Template = { name: "", about: "", title: "", body: "", labels: [], issue_type: null };

function Templates({ repo }: { repo: Repo }) {
  const path = `/repos/${repo.owner}/${repo.name}/issue-templates`;
  const { data, mutate } = useSWR<Template[], ApiError>(path, api);
  const [draft, setDraft] = useState<Template[] | null>(null);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const list = draft ?? data ?? [];
  const edit = (i: number, change: Partial<Template>) => setDraft(list.map((t, j) => (j === i ? { ...t, ...change } : t)));

  async function save() {
    setBusy(true);
    setStatus(null);
    try {
      const cleaned = list.map((t) => ({ ...t, labels: t.labels.map((l) => l.trim()).filter(Boolean) }));
      await mutate(await api<Template[]>(path, { method: "PUT", body: cleaned }), { revalidate: false });
      setDraft(null);
      setStatus({ ok: true, text: "Templates saved." });
    } catch (err) {
      setStatus({ ok: false, text: asApiError(err).summary });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Subhead>Issue templates</Subhead>
      <p className="mb-4 text-sm text-fg-muted">Templates are offered when someone opens a new issue. They can prefill the title, the description, labels and the type.</p>
      {status && (
        <Flash variant={status.ok ? "success" : "danger"} className="mb-4" onDismiss={() => setStatus(null)}>
          {status.text}
        </Flash>
      )}
      <div className="flex flex-col gap-4">
        {list.map((t, i) => (
          <fieldset key={i} className="flex flex-col gap-3 rounded-md border border-line p-4">
            <legend className="sr-only">Template {i + 1}</legend>
            <div className="grid gap-3 md:grid-cols-2">
              <Field label="Template name" htmlFor={`t-name-${i}`} required>
                <TextInput id={`t-name-${i}`} maxLength={100} value={t.name} onChange={(e) => edit(i, { name: e.target.value })} />
              </Field>
              <Field label="About" htmlFor={`t-about-${i}`}>
                <TextInput id={`t-about-${i}`} maxLength={200} placeholder="What this template is for" value={t.about} onChange={(e) => edit(i, { about: e.target.value })} />
              </Field>
              <Field label="Issue title" htmlFor={`t-title-${i}`}>
                <TextInput id={`t-title-${i}`} maxLength={256} placeholder="[Bug]: " value={t.title} onChange={(e) => edit(i, { title: e.target.value })} />
              </Field>
              <Field label="Type" htmlFor={`t-type-${i}`}>
                <select id={`t-type-${i}`} className={inputClass} value={t.issue_type ?? ""} onChange={(e) => edit(i, { issue_type: e.target.value || null })}>
                  <option value="">No type</option>
                  {ISSUE_TYPES.map((k) => (
                    <option key={k} value={k}>
                      {k}
                    </option>
                  ))}
                </select>
              </Field>
            </div>
            <Field label="Labels" htmlFor={`t-labels-${i}`} caption="Comma separated names of existing labels.">
              <TextInput
                id={`t-labels-${i}`}
                placeholder="bug, help wanted"
                value={t.labels.join(", ")}
                onChange={(e) =>
                  edit(i, {
                    labels: e.target.value
                      .split(",")
                      .map((l) => l.trimStart())
                      .filter((l, j, all) => l !== "" || j === all.length - 1),
                  })
                }
              />
            </Field>
            <Field label="Description" htmlFor={`t-body-${i}`} caption="Markdown. This becomes the starting text of the issue.">
              <Textarea id={`t-body-${i}`} rows={6} className="font-mono text-xs" value={t.body} onChange={(e) => edit(i, { body: e.target.value })} />
            </Field>
            <div>
              <Button variant="danger" size="sm" leading={<TrashIcon size={16} />} onClick={() => setDraft(list.filter((_, j) => j !== i))}>
                Remove template
              </Button>
            </div>
          </fieldset>
        ))}
      </div>
      <div className="mt-4 flex flex-wrap gap-2">
        <Button leading={<PlusIcon size={16} />} onClick={() => setDraft([...list, { ...BLANK }])}>
          Add template
        </Button>
        <Button
          variant="primary"
          disabled={busy || draft === null || list.some((t) => !t.name.trim())}
          onClick={save}
        >
          Save templates
        </Button>
      </div>
    </>
  );
}
