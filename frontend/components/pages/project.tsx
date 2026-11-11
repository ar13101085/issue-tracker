"use client";

import { KebabHorizontalIcon, PlusIcon, ProjectIcon, TableIcon, XIcon } from "@primer/octicons-react";
import { useSearchParams } from "next/navigation";
import { type DragEvent, type FormEvent, useState } from "react";
import useSWR from "swr";
import { AvatarStack, TypeBadge } from "@/components/issue/pickers";
import { PageError } from "@/components/pages/repos";
import { CellButton, OptionDot, optionItems, OptionPill } from "@/components/project/parts";
import { ActionDivider, ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { Button, buttonClass } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Field, Flash, inputClass, TextInput } from "@/components/ui/form";
import { IssueLabel } from "@/components/ui/issue-label";
import { IssueStateIcon, useTitle } from "@/components/ui/misc";
import { SelectPanel } from "@/components/ui/select-panel";
import { api, ApiError, asApiError, type FieldKind, type IssueList, type Project, type ProjectField, type ProjectItem, STATUS_FIELD } from "@/lib/api";
import { Link, navigate } from "@/lib/router";
import { cn } from "@/lib/utils";

const KINDS: { kind: FieldKind; label: string }[] = [
  { kind: "text", label: "Text" },
  { kind: "number", label: "Number" },
  { kind: "date", label: "Date" },
  { kind: "single_select", label: "Single select" },
];

type Dialogs =
  | { kind: "field" }
  | { kind: "options"; field: ProjectField }
  | { kind: "rename-field"; field: ProjectField }
  | { kind: "rename" }
  | { kind: "delete" }
  | null;

export function ProjectPage({ owner, repo, number }: { owner: string; repo: string; number: number }) {
  const full = `${owner}/${repo}`;
  const path = `/repos/${full}/projects/${number}`;
  const base = `/${full}/projects/${number}`;
  const search = useSearchParams();
  const view = search.get("view") === "board" ? "board" : "table";
  const { data: project, error, mutate } = useSWR<Project, ApiError>(path, api);
  const [failure, setFailure] = useState<string | null>(null);
  const [dialog, setDialog] = useState<Dialogs>(null);
  useTitle(project ? `${project.title} · ${full}` : `Project · ${full}`);

  if (error) return <PageError error={error} />;
  if (!project) return null;

  /** Sends a change; the server answers with the whole project. */
  async function send(sub: string, method: string, body?: unknown, optimistic?: Project) {
    setFailure(null);
    try {
      await mutate(api<Project>(`${path}${sub}`, { method, body }), optimistic ? { optimisticData: optimistic, rollbackOnError: true, revalidate: false } : { revalidate: false });
      return true;
    } catch (err) {
      setFailure(asApiError(err).summary);
      return false;
    }
  }

  const status = project.fields.find((f) => f.id === STATUS_FIELD)!;
  const custom = project.fields.filter((f) => f.id !== STATUS_FIELD);
  const setValue = (item: ProjectItem, fieldId: string, value: string | number | null) => {
    const values = { ...item.values };
    if (value === null || value === "") delete values[fieldId];
    else values[fieldId] = value;
    const optimistic = { ...project, items: project.items.map((i) => (i.id === item.id ? { ...i, values } : i)) };
    return send(`/items/${item.id}`, "PATCH", { values: { [fieldId]: value === "" ? null : value } }, optimistic);
  };

  const viewTab = (key: "table" | "board", label: string, icon: React.ReactNode) => (
    <Link
      href={key === "table" ? base : `${base}?view=board`}
      aria-current={view === key ? "page" : undefined}
      className={cn("-mb-px flex h-[37px] items-center gap-2 rounded-t-md border border-b-0 px-4 text-sm leading-5 whitespace-nowrap", view === key ? "border-line bg-canvas text-fg" : "border-transparent text-fg-muted hover:text-fg")}
    >
      {icon}
      {label}
    </Link>
  );

  return (
    <div className="flex min-h-[calc(100vh-110px)] flex-col">
      <div className="flex min-h-12 flex-wrap items-center gap-2 bg-canvas-muted px-4 py-2">
        <div className="flex min-w-0 flex-1 items-center gap-1">
          <Link href={`/${full}/projects`} aria-label="All projects" title="All projects" className="flex h-5 w-4 shrink-0 items-center justify-center text-fg-muted hover:text-fg-accent">
            <TableIcon size={16} />
          </Link>
          <h1 className="min-w-0 truncate text-xl leading-[30px] font-semibold" title={project.description || undefined}>
            {project.title}
          </h1>
          {project.closed && <span className="ml-2 rounded-full bg-done-emphasis px-2 py-0.5 text-xs font-medium text-fg-on-emphasis">Closed</span>}
        </div>
        {project.viewer_can_edit && <AddItem repo={full} project={project} onAdd={(n) => send("/items", "POST", { issue_number: n })} />}
        {project.viewer_can_admin && (
          <ActionMenu
            trigger={
              <button type="button" aria-label="Project menu" className={cn(buttonClass("default"), "px-2")}>
                <KebabHorizontalIcon size={16} />
              </button>
            }
          >
            <ActionItem onSelect={() => setDialog({ kind: "rename" })}>Rename project</ActionItem>
            <ActionItem onSelect={() => setDialog({ kind: "field" })}>New field</ActionItem>
            <ActionItem onSelect={() => setDialog({ kind: "options", field: status })}>Edit status columns</ActionItem>
            <ActionItem onSelect={() => send("", "PATCH", { closed: !project.closed })}>{project.closed ? "Reopen project" : "Close project"}</ActionItem>
            <ActionDivider />
            <ActionItem danger onSelect={() => setDialog({ kind: "delete" })}>
              Delete project
            </ActionItem>
          </ActionMenu>
        )}
      </div>

      {failure && (
        <Flash className="mx-4 my-2" onDismiss={() => setFailure(null)}>
          {failure}
        </Flash>
      )}

      <div className="flex overflow-x-auto border-b border-line bg-canvas-muted px-4">
        {viewTab("table", "Table", <TableIcon size={16} />)}
        {viewTab("board", "Board", <ProjectIcon size={16} />)}
      </div>

      {project.items.length === 0 ? (
        <div className="py-12 text-center">
          <p className="text-base font-semibold">This project has no items yet</p>
          <p className="text-sm text-fg-muted">{project.viewer_can_edit ? "Use “Add item” to bring in issues from this repository." : "Items added by the team will show up here."}</p>
        </div>
      ) : view === "table" ? (
        <TableView
          repo={full}
          project={project}
          custom={custom}
          status={status}
          setValue={setValue}
          onRemove={(item) => send(`/items/${item.id}`, "DELETE", undefined, { ...project, items: project.items.filter((i) => i.id !== item.id) })}
          onFieldMenu={setDialog}
          onDeleteField={(f) => send(`/fields/${f.id}`, "DELETE")}
        />
      ) : (
        <BoardView
          repo={full}
          project={project}
          status={status}
          onMove={(item, optionId, beforeId) => {
            const values = { ...item.values };
            if (optionId) values[STATUS_FIELD] = optionId;
            else delete values[STATUS_FIELD];
            // Show the card in its new place at once; the server answer then settles the order.
            const rest = project.items.filter((i) => i.id !== item.id);
            const at = beforeId ? rest.findIndex((i) => i.id === beforeId) : -1;
            const moved = { ...item, values };
            const items = at === -1 ? [...rest, moved] : [...rest.slice(0, at), moved, ...rest.slice(at)];
            return send(`/items/${item.id}`, "PATCH", { values: { [STATUS_FIELD]: optionId }, before: beforeId }, { ...project, items });
          }}
          onRemove={(item) => send(`/items/${item.id}`, "DELETE", undefined, { ...project, items: project.items.filter((i) => i.id !== item.id) })}
        />
      )}

      <FieldDialog open={dialog?.kind === "field"} onClose={() => setDialog(null)} onCreate={(body) => send("/fields", "POST", body)} />
      {dialog?.kind === "options" && (
        <OptionsDialog field={dialog.field} onClose={() => setDialog(null)} onSave={(options) => send(`/fields/${dialog.field.id}`, "PATCH", { options })} />
      )}
      {(dialog?.kind === "rename" || dialog?.kind === "rename-field") && (
        <RenameDialog
          title={dialog.kind === "rename" ? "Rename project" : "Rename field"}
          initial={dialog.kind === "rename" ? project.title : dialog.field.name}
          onClose={() => setDialog(null)}
          onSave={(name) => (dialog.kind === "rename" ? send("", "PATCH", { title: name }) : send(`/fields/${dialog.field.id}`, "PATCH", { name }))}
        />
      )}
      <Dialog
        open={dialog?.kind === "delete"}
        onClose={() => setDialog(null)}
        title="Delete project"
        footer={
          <>
            <Button onClick={() => setDialog(null)}>Cancel</Button>
            <Button
              variant="danger"
              onClick={async () => {
                try {
                  await api(path, { method: "DELETE" });
                  navigate(`/${full}/projects`);
                } catch (err) {
                  setFailure(asApiError(err).summary);
                  setDialog(null);
                }
              }}
            >
              Delete this project
            </Button>
          </>
        }
      >
        Deleting <strong>{project.title}</strong> removes its fields and the place of every item on the board. The issues themselves are kept. This cannot be undone.
      </Dialog>
    </div>
  );
}

function AddItem({ repo, project, onAdd }: { repo: string; project: Project; onAdd: (issueNumber: number) => void }) {
  const candidates = useSWR<IssueList>(`/repos/${repo}/issues?q=${encodeURIComponent("is:issue state:open")}&per_page=100`, api);
  const taken = new Set(project.items.map((i) => i.issue.number));
  const items = (candidates.data?.items ?? [])
    .filter((i) => !taken.has(i.number))
    .map((i) => ({
      key: String(i.number),
      text: `${i.title} #${i.number}`,
      node: (
        <>
          <IssueStateIcon issue={i} />
          <span className="min-w-0 flex-1 truncate">{i.title}</span>
          <span className="text-fg-muted">#{i.number}</span>
        </>
      ),
    }));
  return (
    <SelectPanel
      title="Add an issue to this project"
      placeholder="Search open issues"
      loading={!candidates.data}
      empty="No open issues left to add"
      trigger={
        <button type="button" className={buttonClass("primary")}>
          <PlusIcon size={16} />
          Add item
        </button>
      }
      items={items}
      selected={[]}
      onToggle={(key) => onAdd(Number(key))}
    />
  );
}

/** Text, number and date cells: edited in place, saved on blur or Enter. */
function InlineValue({ field, value, editable, onSave }: { field: ProjectField; value: string | number | undefined; editable: boolean; onSave: (v: string | number | null) => void }) {
  const [draft, setDraft] = useState<string | null>(null);
  const shown = value === undefined ? "" : String(value);
  if (!editable) return <span className="block truncate px-3">{shown}</span>;
  const commit = () => {
    if (draft === null) return;
    const text = draft.trim();
    setDraft(null);
    if (text === shown) return;
    if (text === "") onSave(null);
    else if (field.kind === "number") {
      const n = Number(text);
      if (Number.isFinite(n)) onSave(n);
    } else onSave(text);
  };
  return (
    <input
      aria-label={field.name}
      type={field.kind === "date" ? "date" : field.kind === "number" ? "number" : "text"}
      step="any"
      value={draft ?? shown}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") setDraft(null);
      }}
      className="h-9 w-full bg-transparent px-3 text-sm outline-none hover:bg-control-transparent-hover focus:bg-canvas focus:outline-2 focus:-outline-offset-2 focus:outline-[var(--focus-outlineColor)]"
    />
  );
}

function TableView({
  repo,
  project,
  custom,
  status,
  setValue,
  onRemove,
  onFieldMenu,
  onDeleteField,
}: {
  repo: string;
  project: Project;
  custom: ProjectField[];
  status: ProjectField;
  setValue: (item: ProjectItem, fieldId: string, value: string | number | null) => void;
  onRemove: (item: ProjectItem) => void;
  onFieldMenu: (d: Dialogs) => void;
  onDeleteField: (f: ProjectField) => void;
}) {
  const th = "h-[34px] border-b-2 border-r border-line bg-canvas px-3 text-left text-xs font-semibold whitespace-nowrap text-fg-muted";
  const td = "border-b border-r border-line-muted p-0 align-middle";
  const select = (item: ProjectItem, field: ProjectField) => {
    const current = field.options.find((o) => o.id === item.values[field.id]);
    if (!project.viewer_can_edit) return <span className="block px-3">{current && <OptionPill option={current} />}</span>;
    return (
      <SelectPanel
        align="start"
        title={`Select ${field.name.toLowerCase()}`}
        placeholder="Filter options"
        trigger={<CellButton aria-label={`${field.name} of issue ${item.issue.number}`}>{current && <OptionPill option={current} />}</CellButton>}
        items={optionItems(field.options)}
        selected={current ? [current.id] : []}
        onApply={([key]) => setValue(item, field.id, key ?? null)}
      />
    );
  };
  return (
    <div className="overflow-x-auto border-b border-line-muted">
      <table className="w-full min-w-max border-collapse text-sm">
        <thead>
          <tr>
            <th className={cn(th, "w-[60px]")} aria-label="Row" />
            <th className={cn(th, "min-w-[280px]")}>Title</th>
            <th className={th}>Assignees</th>
            <th className={th}>Status</th>
            <th className={th}>Labels</th>
            <th className={th}>Milestone</th>
            {custom.map((f) => (
              <th key={f.id} className={th}>
                <span className="flex items-center justify-between gap-2">
                  {f.name}
                  {project.viewer_can_admin && (
                    <ActionMenu
                      trigger={
                        <button type="button" aria-label={`${f.name} field menu`} className="flex h-6 w-6 cursor-pointer items-center justify-center rounded-md hover:bg-control-transparent-hover">
                          <KebabHorizontalIcon size={16} />
                        </button>
                      }
                    >
                      <ActionItem onSelect={() => onFieldMenu({ kind: "rename-field", field: f })}>Rename field</ActionItem>
                      {f.kind === "single_select" && <ActionItem onSelect={() => onFieldMenu({ kind: "options", field: f })}>Edit options</ActionItem>}
                      <ActionItem danger onSelect={() => onDeleteField(f)}>
                        Delete field
                      </ActionItem>
                    </ActionMenu>
                  )}
                </span>
              </th>
            ))}
            <th className={cn(th, "w-10 border-r-0 p-0")}>
              {project.viewer_can_admin && (
                <button type="button" aria-label="New field" title="New field" onClick={() => onFieldMenu({ kind: "field" })} className="flex h-8 w-10 cursor-pointer items-center justify-center hover:bg-control-transparent-hover">
                  <PlusIcon size={16} />
                </button>
              )}
            </th>
          </tr>
        </thead>
        <tbody>
          {project.items.map((item, index) => (
            <tr key={item.id} className="group h-[37px] hover:bg-canvas-muted">
              <td className={cn(td, "pr-1 pl-7 text-fg-muted")}>{index + 1}</td>
              <td className={td}>
                <span className="flex min-h-9 items-center gap-1 pr-3 pl-[35px]">
                  <IssueStateIcon issue={item.issue} />
                  <Link href={`/${repo}/issues/${item.issue.number}`} className="min-w-0 max-w-[420px] truncate hover:text-fg-accent hover:underline">
                    {item.issue.title}
                  </Link>
                  <span className="ml-1 text-fg-muted">#{item.issue.number}</span>
                  {item.issue.issue_type && <TypeBadge type={item.issue.issue_type} />}
                </span>
              </td>
              <td className={td}>
                <span className="flex items-center gap-2 px-3">
                  {item.issue.assignees.length > 0 && <AvatarStack logins={item.issue.assignees.map((a) => a.login)} />}
                  <span className="truncate">{item.issue.assignees.map((a) => a.login).join(", ")}</span>
                </span>
              </td>
              <td className={cn(td, "min-w-[140px]")}>{select(item, status)}</td>
              <td className={td}>
                <span className="flex gap-1 px-3">
                  {item.issue.labels.map((l) => (
                    <IssueLabel key={l.name} label={l} />
                  ))}
                </span>
              </td>
              <td className={cn(td, "px-3 whitespace-nowrap")}>{item.issue.milestone?.title}</td>
              {custom.map((f) => (
                <td key={f.id} className={cn(td, "min-w-[140px]")}>
                  {f.kind === "single_select" ? select(item, f) : <InlineValue field={f} value={item.values[f.id]} editable={project.viewer_can_edit} onSave={(v) => setValue(item, f.id, v)} />}
                </td>
              ))}
              <td className={cn(td, "border-r-0")}>
                {project.viewer_can_edit && (
                  <button type="button" aria-label={`Remove issue ${item.issue.number} from project`} title="Remove from project" onClick={() => onRemove(item)} className="flex h-9 w-10 cursor-pointer items-center justify-center text-fg-muted hover:text-fg-danger">
                    <XIcon size={16} />
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function BoardView({
  repo,
  project,
  status,
  onMove,
  onRemove,
}: {
  repo: string;
  project: Project;
  status: ProjectField;
  onMove: (item: ProjectItem, optionId: string | null, beforeId: string | null) => void;
  onRemove: (item: ProjectItem) => void;
}) {
  const [dragging, setDragging] = useState<string | null>(null);
  // Where the dragged card would land: column key and the card it would sit before.
  const [target, setTarget] = useState<{ column: string; before: string | null } | null>(null);
  const columns = [{ id: "", name: "No Status", color: "gray" }, ...status.options];
  const inColumn = (id: string) => project.items.filter((i) => ((i.values[STATUS_FIELD] as string | undefined) ?? "") === id);

  function over(e: DragEvent, column: string, cards: ProjectItem[]) {
    if (!dragging) return;
    e.preventDefault();
    // The first card whose middle is below the pointer is the one to sit before.
    const nodes = [...e.currentTarget.querySelectorAll<HTMLElement>("[data-card]")];
    const next = nodes.find((n) => n.dataset.card !== dragging && e.clientY < n.getBoundingClientRect().top + n.offsetHeight / 2);
    const before = next?.dataset.card ?? null;
    if (target?.column !== column || target.before !== before) setTarget({ column, before });
    void cards;
  }

  function drop(e: DragEvent, column: string) {
    e.preventDefault();
    const item = project.items.find((i) => i.id === dragging);
    const before = target?.column === column ? target.before : null;
    setDragging(null);
    setTarget(null);
    if (!item) return;
    const from = (item.values[STATUS_FIELD] as string | undefined) ?? "";
    const cards = inColumn(column);
    const index = cards.findIndex((c) => c.id === item.id);
    const unchanged = from === column && (before === (cards[index + 1]?.id ?? null) || before === item.id);
    if (!unchanged) onMove(item, column || null, before);
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-x-auto px-4 pt-2 pb-4">
      {columns.map((col) => {
        const cards = inColumn(col.id);
        if (col.id === "" && cards.length === 0 && !dragging) return null;
        return (
          <section
            key={col.id || "none"}
            aria-label={col.name}
            onDragOver={(e) => over(e, col.id, cards)}
            onDrop={(e) => drop(e, col.id)}
            className={cn("mr-2 flex max-h-[calc(100vh-190px)] min-h-[160px] w-[350px] max-w-[85vw] shrink-0 flex-col rounded-md border bg-canvas-muted", target?.column === col.id ? "border-[var(--borderColor-accent-emphasis)]" : "border-line")}
          >
            <header className="flex min-h-11 items-center gap-2 pt-2 pr-2 pb-1 pl-4 text-sm">
              <OptionDot color={col.color} />
              <h2 className="truncate font-semibold">{col.name}</h2>
              <span className="inline-flex h-[18px] items-center rounded-full bg-neutral-muted px-[7px] text-xs font-semibold text-fg-muted">{cards.length}</span>
            </header>
            <ul className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-2 pt-2 pb-1">
              {cards.map((item) => (
                <li key={item.id} data-card={item.id}>
                  {target?.column === col.id && target.before === item.id && <div className="mb-2 h-0.5 rounded bg-accent-emphasis" />}
                  <article
                    draggable={project.viewer_can_edit}
                    onDragStart={(e) => {
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", item.id);
                      setDragging(item.id);
                    }}
                    onDragEnd={() => {
                      setDragging(null);
                      setTarget(null);
                    }}
                    className={cn("rounded-md border border-line bg-canvas px-3 pt-2 pb-3 shadow-[var(--shadow-resting-small)]", project.viewer_can_edit && "cursor-grab active:cursor-grabbing", dragging === item.id && "opacity-40")}
                  >
                    <div className="mb-1 flex min-h-6 items-center gap-1">
                      <IssueStateIcon issue={item.issue} />
                      <span className="min-w-0 flex-1 truncate text-xs text-fg-muted">
                        {repo.split("/")[1]} #{item.issue.number}
                      </span>
                      {project.viewer_can_edit && (
                        <ActionMenu
                          trigger={
                            <button type="button" aria-label={`Card menu for issue ${item.issue.number}`} className="flex h-5 w-6 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
                              <KebabHorizontalIcon size={16} />
                            </button>
                          }
                        >
                          {columns
                            .filter((c) => c.id !== col.id)
                            .map((c) => (
                              <ActionItem key={c.id || "none"} leading={<OptionDot color={c.color} />} onSelect={() => onMove(item, c.id || null, null)}>
                                Move to {c.name}
                              </ActionItem>
                            ))}
                          <ActionDivider />
                          <ActionItem danger onSelect={() => onRemove(item)}>
                            Remove from project
                          </ActionItem>
                        </ActionMenu>
                      )}
                    </div>
                    <Link href={`/${repo}/issues/${item.issue.number}`} draggable={false} className="block text-sm leading-[1.3] break-words hover:text-fg-accent hover:underline">
                      {item.issue.title}
                    </Link>
                    {(item.issue.labels.length > 0 || item.issue.assignees.length > 0 || item.issue.issue_type) && (
                      <div className="mt-2 flex flex-wrap items-center gap-1">
                        {item.issue.issue_type && <TypeBadge type={item.issue.issue_type} />}
                        {item.issue.labels.map((l) => (
                          <IssueLabel key={l.name} label={l} />
                        ))}
                        {item.issue.assignees.length > 0 && (
                          <span className="ml-auto">
                            <AvatarStack logins={item.issue.assignees.map((a) => a.login)} />
                          </span>
                        )}
                      </div>
                    )}
                  </article>
                </li>
              ))}
              {target?.column === col.id && target.before === null && dragging && <li className="h-0.5 rounded bg-accent-emphasis" />}
            </ul>
          </section>
        );
      })}
    </div>
  );
}

function FieldDialog({ open, onClose, onCreate }: { open: boolean; onClose: () => void; onCreate: (body: { name: string; kind: FieldKind; options: string[] }) => Promise<boolean> }) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<FieldKind>("text");
  const [options, setOptions] = useState("");
  const [busy, setBusy] = useState(false);
  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    const list = options.split("\n").map((o) => o.trim()).filter(Boolean);
    if (await onCreate({ name, kind, options: kind === "single_select" ? list : [] })) {
      setName("");
      setOptions("");
      setKind("text");
    }
    setBusy(false);
    onClose();
  }
  return (
    <Dialog open={open} onClose={onClose} title="New field">
      <form onSubmit={submit} className="flex flex-col gap-4">
        <Field label="Field name" htmlFor="field-name" required>
          <TextInput id="field-name" autoFocus maxLength={50} value={name} onChange={(e) => setName(e.target.value)} />
        </Field>
        <Field label="Field type" htmlFor="field-kind">
          <select id="field-kind" className={inputClass} value={kind} onChange={(e) => setKind(e.target.value as FieldKind)}>
            {KINDS.map((k) => (
              <option key={k.kind} value={k.kind}>
                {k.label}
              </option>
            ))}
          </select>
        </Field>
        {kind === "single_select" && (
          <Field label="Options" htmlFor="field-options" caption="One option per line.">
            <textarea id="field-options" rows={4} className={cn(inputClass, "h-auto py-2")} value={options} onChange={(e) => setOptions(e.target.value)} />
          </Field>
        )}
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={busy || !name.trim() || (kind === "single_select" && !options.trim())}>
            Save
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function OptionsDialog({ field, onClose, onSave }: { field: ProjectField; onClose: () => void; onSave: (options: { id?: string; name: string; color?: string }[]) => Promise<boolean> }) {
  const [rows, setRows] = useState<{ id?: string; name: string; color?: string }[]>(field.options);
  const [busy, setBusy] = useState(false);
  const valid = rows.length > 0 && rows.every((r) => r.name.trim());
  return (
    <Dialog
      open
      onClose={onClose}
      title={field.id === STATUS_FIELD ? "Edit status columns" : `Edit ${field.name} options`}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            disabled={busy || !valid}
            onClick={async () => {
              setBusy(true);
              await onSave(rows);
              onClose();
            }}
          >
            Save options
          </Button>
        </>
      }
    >
      <ul className="flex flex-col gap-2">
        {rows.map((row, i) => (
          <li key={row.id ?? `new-${i}`} className="flex items-center gap-2">
            <OptionDot color={row.color ?? "gray"} />
            <TextInput aria-label={`Option ${i + 1}`} maxLength={50} value={row.name} onChange={(e) => setRows(rows.map((r, j) => (j === i ? { ...r, name: e.target.value } : r)))} />
            <button type="button" aria-label={`Remove option ${row.name || i + 1}`} disabled={rows.length === 1} onClick={() => setRows(rows.filter((_, j) => j !== i))} className="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover hover:text-fg-danger disabled:opacity-40">
              <XIcon size={16} />
            </button>
          </li>
        ))}
      </ul>
      <Button className="mt-3" size="sm" leading={<PlusIcon size={16} />} onClick={() => setRows([...rows, { name: "" }])}>
        Add option
      </Button>
      <p className="mt-3 text-xs text-fg-muted">Items using an option you remove go back to having no value.</p>
    </Dialog>
  );
}

function RenameDialog({ title, initial, onClose, onSave }: { title: string; initial: string; onClose: () => void; onSave: (name: string) => Promise<boolean> }) {
  const [name, setName] = useState(initial);
  return (
    <Dialog open onClose={onClose} title={title}>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          await onSave(name);
          onClose();
        }}
        className="flex flex-col gap-4"
      >
        <TextInput aria-label="Name" autoFocus maxLength={255} value={name} onChange={(e) => setName(e.target.value)} />
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={!name.trim()}>
            Save
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
