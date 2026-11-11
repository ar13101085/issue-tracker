"use client";

import type { ButtonHTMLAttributes, ReactNode } from "react";
import type { PanelItem } from "@/components/ui/select-panel";
import type { FieldOption } from "@/lib/api";
import { cn } from "@/lib/utils";

// Option colors in Primer's roles: [dot, pill background, pill text].
const TONES: Record<string, [string, string, string]> = {
  gray: ["bg-[var(--fgColor-muted)]", "bg-neutral-muted", "text-fg-muted"],
  blue: ["bg-[var(--fgColor-accent)]", "bg-[var(--bgColor-accent-muted)]", "text-fg-accent"],
  green: ["bg-[var(--fgColor-success)]", "bg-[var(--bgColor-success-muted)]", "text-fg-success"],
  yellow: ["bg-[var(--fgColor-attention)]", "bg-[var(--bgColor-attention-muted)]", "text-fg-attention"],
  orange: ["bg-[var(--fgColor-severe)]", "bg-[var(--bgColor-severe-muted)]", "text-[var(--fgColor-severe)]"],
  red: ["bg-[var(--fgColor-danger)]", "bg-[var(--bgColor-danger-muted)]", "text-fg-danger"],
  pink: ["bg-[var(--fgColor-sponsors)]", "bg-[var(--bgColor-sponsors-muted)]", "text-[var(--fgColor-sponsors)]"],
  purple: ["bg-[var(--fgColor-done)]", "bg-[var(--bgColor-done-muted)]", "text-fg-done"],
};

export function OptionDot({ color }: { color: string }) {
  return <span className={cn("h-3 w-3 shrink-0 rounded-full", (TONES[color] ?? TONES.gray)[0])} />;
}

export function OptionPill({ option }: { option: FieldOption }) {
  const [, bg, fg] = TONES[option.color] ?? TONES.gray;
  return <span className={cn("inline-block h-5 max-w-full truncate rounded-full px-2 text-xs leading-5 font-medium", bg, fg)}>{option.name}</span>;
}

export function optionItems(options: FieldOption[]): PanelItem[] {
  return options.map((o) => ({
    key: o.id,
    text: o.name,
    node: (
      <>
        <OptionDot color={o.color} />
        <span className="truncate">{o.name}</span>
      </>
    ),
  }));
}

/** A table cell that opens a picker: looks like plain content until hovered. */
export function CellButton({ children, ...rest }: { children: ReactNode } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button type="button" {...rest} className="flex min-h-9 w-full cursor-pointer items-center gap-1 px-3 text-left hover:bg-control-transparent-hover">
      {children}
    </button>
  );
}
