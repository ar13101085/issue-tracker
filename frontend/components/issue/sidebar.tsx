"use client";

import { BellIcon, BellSlashIcon, GearIcon } from "@primer/octicons-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import useSWR from "swr";
import { OptionPill } from "@/components/project/parts";
import { api, type IssueProject, type ProjectSummary } from "@/lib/api";
import { labelItems, milestoneItems, TypeBadge, typeItems, useAssignees, useLabels, useMilestones, userItems } from "@/components/issue/pickers";
import { Progress } from "@/components/pages/milestones";
import { Avatar } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { IssueLabel } from "@/components/ui/issue-label";
import { SelectPanel } from "@/components/ui/select-panel";
import { searchHref, setToken, toggleToken } from "@/lib/query";
import { Link } from "@/lib/router";

export function SidebarSection({ title, action, children }: { title: string; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="border-b border-line-muted py-4 text-xs first:pt-0 last:border-0">
      <div className="mb-2 flex min-h-6 items-center justify-between font-semibold text-fg-muted">
        <h3>{title}</h3>
        {action}
      </div>
      {children}
    </section>
  );
}

// The select panel hands its click handler and ref to the trigger, so they must reach the button.
export function Gear({ label, ...rest }: { label: string } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button type="button" {...rest} aria-label={label} className="flex h-6 w-6 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover hover:text-fg-accent">
      <GearIcon size={16} />
    </button>
  );
}

export type Metadata = { assignees: string[]; labels: string[]; type: string | null; milestone: string | null };

/**
 * Assignees, labels, type and milestone. On an existing issue each change is
 * saved by the caller; on the new issue page it only updates the draft.
 */
export function MetadataSections({
  repo,
  value,
  canEdit,
  viewer,
  onChange,
  beforeMilestone,
}: {
  repo: string;
  value: Metadata;
  canEdit: boolean;
  viewer?: string;
  onChange: (change: Partial<Metadata>) => void;
  /** Rendered between Type and Milestone: the Projects section on an existing issue. */
  beforeMilestone?: ReactNode;
}) {
  const labels = useLabels(repo);
  const milestones = useMilestones(repo);
  const assignees = useAssignees(repo, canEdit);
  const base = `/${repo}/issues`;
  const chosenLabels = (labels.data ?? []).filter((l) => value.labels.includes(l.name));
  const milestone = milestones.data?.find((m) => m.title === value.milestone);
  const canSelfAssign = canEdit && !!viewer && (assignees.data ?? []).some((u) => u.login === viewer);

  return (
    <>
      <SidebarSection
        title="Assignees"
        action={
          canEdit && (
            <SelectPanel
              title="Assign up to 10 people to this issue"
              placeholder="Filter assignees"
              multi
              loading={!assignees.data}
              trigger={<Gear label="Edit assignees" />}
              items={userItems(assignees.data)}
              selected={value.assignees}
              onApply={(keys) => onChange({ assignees: keys })}
            />
          )
        }
      >
        {value.assignees.length === 0 ? (
          <p className="text-fg-muted">
            No one assigned
            {canSelfAssign && (
              <>
                {" – "}
                <button type="button" onClick={() => onChange({ assignees: [viewer!] })} className="cursor-pointer text-fg-accent hover:underline">
                  Assign yourself
                </button>
              </>
            )}
          </p>
        ) : (
          <ul className="flex flex-col gap-2">
            {value.assignees.map((login) => (
              <li key={login}>
                <Link href={`/${login}`} className="flex items-center gap-2 text-sm font-semibold hover:text-fg-accent">
                  <Avatar login={login} size={20} />
                  {login}
                </Link>
              </li>
            ))}
          </ul>
        )}
      </SidebarSection>

      <SidebarSection
        title="Labels"
        action={
          canEdit && (
            <SelectPanel
              title="Apply labels to this issue"
              placeholder="Filter labels"
              multi
              loading={!labels.data}
              trigger={<Gear label="Edit labels" />}
              items={labelItems(labels.data)}
              selected={value.labels}
              onApply={(keys) => onChange({ labels: keys })}
              footer={
                <Link href={`/${repo}/labels`} className="block rounded-md px-2 py-1.5 text-sm hover:bg-control-transparent-hover">
                  Edit labels
                </Link>
              }
            />
          )
        }
      >
        {chosenLabels.length === 0 ? (
          <p className="text-fg-muted">No labels</p>
        ) : (
          <div className="flex flex-wrap gap-1">
            {chosenLabels.map((l) => (
              <IssueLabel key={l.name} label={l} href={searchHref(base, toggleToken("is:issue state:open", "label", l.name))} />
            ))}
          </div>
        )}
      </SidebarSection>

      <SidebarSection
        title="Type"
        action={
          canEdit && (
            <SelectPanel title="Select issue type" placeholder="Filter types" trigger={<Gear label="Edit type" />} items={typeItems()} selected={value.type ? [value.type] : []} onApply={([key]) => onChange({ type: key ?? null })} />
          )
        }
      >
        {value.type ? <TypeBadge type={value.type} /> : <p className="text-fg-muted">No type</p>}
      </SidebarSection>

      {beforeMilestone}

      <SidebarSection
        title="Milestone"
        action={
          canEdit && (
            <SelectPanel
              title="Set milestone"
              placeholder="Filter milestones"
              loading={!milestones.data}
              trigger={<Gear label="Edit milestone" />}
              items={milestoneItems(milestones.data, true)}
              selected={value.milestone ? [value.milestone] : []}
              onApply={([key]) => onChange({ milestone: key ?? null })}
              empty="No open milestones"
              footer={
                <Link href={`/${repo}/milestones/new`} className="block rounded-md px-2 py-1.5 text-sm hover:bg-control-transparent-hover">
                  Create a milestone
                </Link>
              }
            />
          )
        }
      >
        {value.milestone ? (
          <>
            <Link href={searchHref(base, setToken("is:issue state:open", "milestone", value.milestone))} className="text-sm font-semibold hover:text-fg-accent">
              {value.milestone}
            </Link>
            {milestone && <Progress open={milestone.open_issues} closed={milestone.closed_issues} className="mt-2" />}
          </>
        ) : (
          <p className="text-fg-muted">No milestone</p>
        )}
      </SidebarSection>
    </>
  );
}

export function SubscribeSection({ subscribed, onToggle, busy }: { subscribed: boolean; onToggle: () => void; busy: boolean }) {
  return (
    <SidebarSection title="Notifications">
      <Button block disabled={busy} onClick={onToggle} leading={subscribed ? <BellSlashIcon size={16} /> : <BellIcon size={16} />}>
        {subscribed ? "Unsubscribe" : "Subscribe"}
      </Button>
      <p className="mt-2 text-fg-muted">{subscribed ? "You're receiving notifications because you're subscribed to this thread." : "You're not receiving notifications from this thread."}</p>
    </SidebarSection>
  );
}

export function ParticipantsSection({ participants }: { participants: string[] }) {
  return (
    <SidebarSection title={`${participants.length} participant${participants.length === 1 ? "" : "s"}`}>
      <div className="flex flex-wrap gap-1">
        {participants.map((login) => (
          <Link key={login} href={`/${login}`} title={login}>
            <Avatar login={login} size={26} />
          </Link>
        ))}
      </div>
    </SidebarSection>
  );
}

/** Which projects the issue is on, with its status in each. */
export function ProjectsSection({ repo, number, canEdit }: { repo: string; number: number; canEdit: boolean }) {
  const key = `/repos/${repo}/issues/${number}/projects`;
  const mine = useSWR<IssueProject[]>(key, api);
  const all = useSWR<ProjectSummary[]>(canEdit ? `/repos/${repo}/projects` : null, api);
  const current = mine.data ?? [];

  async function apply(keys: string[]) {
    const wanted = new Set(keys.map(Number));
    const have = new Set(current.map((p) => p.number));
    try {
      for (const n of wanted) if (!have.has(n)) await api(`/repos/${repo}/projects/${n}/items`, { method: "POST", body: { issue_number: number } });
      for (const p of current) if (!wanted.has(p.number)) await api(`/repos/${repo}/projects/${p.number}/items/${p.item_id}`, { method: "DELETE" });
    } finally {
      // Whatever succeeded is what the section shows.
      await mine.mutate();
    }
  }

  const open = (all.data ?? []).filter((p) => !p.closed || current.some((c) => c.number === p.number));
  return (
    <SidebarSection
      title="Projects"
      action={
        canEdit && (
          <SelectPanel
            title="Projects"
            placeholder="Filter projects"
            multi
            loading={!all.data}
            empty="No projects yet"
            trigger={<Gear label="Edit projects" />}
            items={open.map((p) => ({ key: String(p.number), text: p.title, node: <span className="truncate font-semibold">{p.title}</span> }))}
            selected={current.map((p) => String(p.number))}
            onApply={apply}
          />
        )
      }
    >
      {current.length === 0 ? (
        <p className="text-fg-muted">No projects</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {current.map((p) => (
            <li key={p.number} className="flex items-center justify-between gap-2">
              <Link href={`/${repo}/projects/${p.number}`} className="min-w-0 truncate text-sm font-semibold hover:text-fg-accent">
                {p.title}
              </Link>
              {p.status ? <OptionPill option={p.status} /> : <span className="text-fg-muted">No status</span>}
            </li>
          ))}
        </ul>
      )}
    </SidebarSection>
  );
}
