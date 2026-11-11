import type { CSSProperties } from "react";
import type { LabelRef } from "@/lib/api";
import { Link } from "@/lib/router";

function hsl(r: number, g: number, b: number): [number, number, number] {
  const [rn, gn, bn] = [r / 255, g / 255, b / 255];
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, Math.round(l * 100)];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === rn ? (gn - bn) / d + (gn < bn ? 6 : 0) : max === gn ? (bn - rn) / d + 2 : (rn - gn) / d + 4;
  return [Math.round(h * 60), Math.round(s * 100), Math.round(l * 100)];
}

export function labelStyle(color: string): CSSProperties {
  const n = parseInt(/^[0-9a-f]{6}$/i.test(color) ? color : "ededed", 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  const [h, s, l] = hsl(r, g, b);
  return { "--label-r": r, "--label-g": g, "--label-b": b, "--label-h": h, "--label-s": s, "--label-l": l } as CSSProperties;
}

/** The colored label chip. With `href` it links to the filtered issue list. */
export function IssueLabel({ label, href }: { label: LabelRef; href?: string }) {
  if (href) {
    return (
      <Link href={href} className="issue-label hover:opacity-85" style={labelStyle(label.color)}>
        {label.name}
      </Link>
    );
  }
  return (
    <span className="issue-label" style={labelStyle(label.color)}>
      {label.name}
    </span>
  );
}
