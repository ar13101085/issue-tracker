"use client";

import { IssueOpenedIcon, KebabHorizontalIcon, SearchIcon, SyncIcon, TagIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import { useLabels } from "@/components/issue/pickers";
import { PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { IssuesLayout, ListBox, ListTabs } from "@/components/shell/issues-layout";
import { ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, inputClass, TextInput } from "@/components/ui/form";
import { IssueLabel } from "@/components/ui/issue-label";
import { useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, hasRole, type Label, type LabelWithCount } from "@/lib/api";
import { searchHref, toggleToken } from "@/lib/query";
import { Link } from "@/lib/router";
import { useRepo } from "@/lib/session";
import { cn } from "@/lib/utils";

// GitHub's suggested label colors.
const PALETTE = ["b60205", "d93f0b", "fbca04", "0e8a16", "006b75", "1d76db", "0052cc", "5319e7", "e99695", "f9d0c4", "fef2c0", "c2e0c6", "bfdadc", "c5def5", "bfd4f2", "d4c5f9"];
const randomColor = () => PALETTE[Math.floor(Math.random() * PALETTE.length)];

function LabelForm({ initial, submitLabel, onSubmit, onCancel }: { initial: Label; submitLabel: string; onSubmit: (l: Label) => Promise<void>; onCancel: () => void }) {
  const [form, setForm] = useState(initial);
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const validColor = /^[0-9a-f]{6}$/i.test(form.color);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await onSubmit(form);
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-3">
      <div>
        <IssueLabel label={{ name: form.name.trim() || "Label preview", color: validColor ? form.color : "ededed" }} />
      </div>
      {error && error.errors.length === 0 && <Flash>{error.message}</Flash>}
      <div className="flex flex-col gap-3 md:flex-row md:items-start">
        <div className="md:w-1/4">
          <Field label="Label name" htmlFor="label-name" required error={error?.field("name")}>
            <TextInput id="label-name" autoFocus maxLength={50} placeholder="Label name" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
          </Field>
        </div>
        <div className="md:flex-1">
          <Field label="Description" htmlFor="label-description" error={error?.field("description")}>
            <TextInput id="label-description" maxLength={100} placeholder="Description (optional)" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} />
          </Field>
        </div>
        <div>
          <Field label="Color" htmlFor="label-color" error={error?.field("color")}>
            <div className="flex gap-2">
              <button
                type="button"
                aria-label="Get a new color"
                onClick={() => setForm({ ...form, color: randomColor() })}
                className="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-md border border-line-muted text-white"
                style={{ background: validColor ? `#${form.color}` : "#ededed" }}
              >
                <SyncIcon size={16} className="mix-blend-difference" />
              </button>
              <TextInput
                id="label-color"
                maxLength={7}
                className="w-24 font-mono"
                aria-invalid={!validColor}
                value={`#${form.color}`}
                onChange={(e) => setForm({ ...form, color: e.target.value.replace(/^#/, "").toLowerCase() })}
              />
            </div>
          </Field>
        </div>
        <div className="flex gap-2 md:mt-6 md:ml-auto">
          <Button onClick={onCancel}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={busy || !form.name.trim() || !validColor}>
            {submitLabel}
          </Button>
        </div>
      </div>
    </form>
  );
}

export function LabelsPage({ owner, repo }: { owner: string; repo: string }) {
  useTitle(`Labels · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const { data: repoData } = useRepo(owner, repo);
  const canWrite = hasRole(repoData, "write");
  const { data, error, mutate } = useLabels(full);
  const [filter, setFilter] = useState("");
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<LabelWithCount | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  if (error) return <PageError error={asApiError(error)} />;
  const needle = filter.trim().toLowerCase();
  const shown = (data ?? []).filter((l) => !needle || `${l.name} ${l.description}`.toLowerCase().includes(needle));
  const path = `/repos/${full}/labels`;

  async function remove() {
    if (!deleting) return;
    try {
      await api(`${path}/${encodeURIComponent(deleting.name)}`, { method: "DELETE" });
      await mutate();
      setDeleting(null);
    } catch (err) {
      setFailure(asApiError(err).summary);
      setDeleting(null);
    }
  }

  const issuesHref = (name: string) => searchHref(`/${owner}/${repo}/issues`, toggleToken("is:issue state:open", "label", name));
  return (
    <IssuesLayout
      owner={owner}
      repo={repo}
      active="labels"
      title="Labels"
      actions={
        canWrite && (
          <Button variant="primary" onClick={() => setCreating(true)}>
            New label
          </Button>
        )
      }
    >
      <div className="flex pb-4">
        <input aria-label="Search all labels" placeholder="Search all labels" value={filter} onChange={(e) => setFilter(e.target.value)} className={cn(inputClass, "min-w-0 flex-1 rounded-r-none px-5")} />
        <span className="-ml-px flex h-8 w-8 shrink-0 items-center justify-center rounded-r-md border border-[var(--control-borderColor-rest)] bg-canvas-muted text-fg-muted">
          <SearchIcon size={16} />
        </span>
      </div>

      {failure && (
        <Flash className="mb-4" onDismiss={() => setFailure(null)}>
          {failure}
        </Flash>
      )}

      {creating && (
        <div className="mb-4 rounded-md border border-line bg-canvas-muted p-4">
          <LabelForm
            initial={{ name: "", description: "", color: randomColor() }}
            submitLabel="Create label"
            onCancel={() => setCreating(false)}
            onSubmit={async (l) => {
              await api(path, { method: "POST", body: l });
              await mutate();
              setCreating(false);
            }}
          />
        </div>
      )}

      <ListBox header={<ListTabs tabs={[{ key: "all", label: "Labels", count: shown.length }]} value="all" />}>
        {data && shown.length === 0 && <Blankslate icon={<TagIcon size={24} />} title="No matching labels" description={needle ? "Try a different search." : "Labels help you organise issues."} />}
        <ul>
          {shown.map((l) => (
            <li key={l.name} className="border-b border-line-muted last:border-b-0">
              {editing === l.name ? (
                <div className="p-4">
                  <LabelForm
                    initial={l}
                    submitLabel="Save changes"
                    onCancel={() => setEditing(null)}
                    onSubmit={async (next) => {
                      await api(`${path}/${encodeURIComponent(l.name)}`, { method: "PATCH", body: { new_name: next.name, color: next.color, description: next.description } });
                      await mutate();
                      setEditing(null);
                    }}
                  />
                </div>
              ) : (
                <div className="grid min-h-14 grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-1 py-2 pr-2 pl-4 text-xs text-fg-muted leading-[18px] md:grid-cols-[minmax(0,330fr)_minmax(0,439fr)_98px_auto]">
                  <div className="flex min-w-0">
                    <IssueLabel label={l} href={issuesHref(l.name)} />
                  </div>
                  <p className="order-3 col-span-2 min-w-0 break-words md:order-none md:col-span-1">{l.description}</p>
                  <div className="order-4 flex md:order-none md:justify-end">
                    {l.open_issues > 0 && (
                      <Link href={issuesHref(l.name)} aria-label={`${l.open_issues} open issue${l.open_issues === 1 ? "" : "s"}`} className="flex items-center gap-1 hover:text-fg-accent">
                        <IssueOpenedIcon size={16} />
                        {l.open_issues}
                      </Link>
                    )}
                  </div>
                  <div className="order-2 flex w-8 justify-end md:order-none">
                    {canWrite && (
                      <ActionMenu
                        width={160}
                        trigger={
                          <button type="button" aria-label={`Actions for label ${l.name}`} className="flex h-8 w-8 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
                            <KebabHorizontalIcon size={16} />
                          </button>
                        }
                      >
                        <ActionItem onSelect={() => setEditing(l.name)}>Edit</ActionItem>
                        <ActionItem danger onSelect={() => setDeleting(l)}>
                          Delete
                        </ActionItem>
                      </ActionMenu>
                    )}
                  </div>
                </div>
              )}
            </li>
          ))}
        </ul>
      </ListBox>

      <Dialog
        open={!!deleting}
        onClose={() => setDeleting(null)}
        title="Delete label"
        footer={
          <>
            <Button onClick={() => setDeleting(null)}>Cancel</Button>
            <Button variant="danger" onClick={remove}>
              Delete
            </Button>
          </>
        }
      >
        Are you sure? Deleting <strong>{deleting?.name}</strong> removes it from every issue. This cannot be undone.
      </Dialog>
    </IssuesLayout>
  );
}
