// Editing helpers for the markdown textarea: toolbar formatting and caret position.

export type Format = "heading" | "bold" | "italic" | "quote" | "code" | "link" | "ol" | "ul" | "task" | "mention" | "reference";

export type EditResult = { value: string; start: number; end: number };

function wrap(value: string, start: number, end: number, before: string, after: string, placeholder = ""): EditResult {
  const selected = value.slice(start, end) || placeholder;
  // Toggling: selection already wrapped → unwrap.
  if (value.slice(start - before.length, start) === before && value.slice(end, end + after.length) === after) {
    return {
      value: value.slice(0, start - before.length) + selected + value.slice(end + after.length),
      start: start - before.length,
      end: end - before.length,
    };
  }
  return {
    value: value.slice(0, start) + before + selected + after + value.slice(end),
    start: start + before.length,
    end: start + before.length + selected.length,
  };
}

function prefixLines(value: string, start: number, end: number, prefix: (i: number) => string): EditResult {
  const lineStart = value.lastIndexOf("\n", start - 1) + 1;
  const lineEndRaw = value.indexOf("\n", end);
  const lineEnd = lineEndRaw === -1 ? value.length : lineEndRaw;
  const lines = value.slice(lineStart, lineEnd).split("\n");
  const first = prefix(0);
  const allPrefixed = lines.every((l, i) => l.startsWith(prefix(i)));
  const next = lines.map((l, i) => (allPrefixed ? l.slice(prefix(i).length) : prefix(i) + l)).join("\n");
  const delta = allPrefixed ? -first.length : first.length;
  return {
    value: value.slice(0, lineStart) + next + value.slice(lineEnd),
    start: Math.max(lineStart, start + delta),
    end: lineStart + next.length,
  };
}

export function applyFormat(value: string, start: number, end: number, format: Format): EditResult {
  switch (format) {
    case "bold":
      return wrap(value, start, end, "**", "**");
    case "italic":
      return wrap(value, start, end, "_", "_");
    case "code":
      return value.slice(start, end).includes("\n") ? wrap(value, start, end, "```\n", "\n```") : wrap(value, start, end, "`", "`");
    case "link": {
      const text = value.slice(start, end);
      const inserted = `[${text}](url)`;
      const urlAt = start + text.length + 3;
      return { value: value.slice(0, start) + inserted + value.slice(end), start: text ? urlAt : start + 1, end: text ? urlAt + 3 : start + 1 };
    }
    case "heading":
      return prefixLines(value, start, end, () => "### ");
    case "quote":
      return prefixLines(value, start, end, () => "> ");
    case "ul":
      return prefixLines(value, start, end, () => "- ");
    case "ol":
      return prefixLines(value, start, end, (i) => `${i + 1}. `);
    case "task":
      return prefixLines(value, start, end, () => "- [ ] ");
    case "mention":
    case "reference": {
      const char = format === "mention" ? "@" : "#";
      const needsSpace = start > 0 && !/\s/.test(value[start - 1]);
      const inserted = (needsSpace ? " " : "") + char;
      return { value: value.slice(0, start) + inserted + value.slice(end), start: start + inserted.length, end: start + inserted.length };
    }
  }
}

/** Quotes text for "Quote reply". */
export function quote(text: string): string {
  return `${text
    .trim()
    .split("\n")
    .map((l) => `> ${l}`)
    .join("\n")}\n\n`;
}

/** The `@word` or `#word` being typed right before the caret, if any. */
export function activeTrigger(value: string, caret: number): { char: "@" | "#"; query: string; from: number } | null {
  const before = value.slice(0, caret);
  const match = /(^|[\s(])([@#])([\w-]*)$/.exec(before);
  if (!match) return null;
  return { char: match[2] as "@" | "#", query: match[3], from: caret - match[3].length - 1 };
}

const MIRRORED = [
  "boxSizing", "width", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "borderTopWidth", "borderLeftWidth",
  "fontFamily", "fontSize", "fontWeight", "lineHeight", "letterSpacing", "tabSize", "whiteSpace", "wordBreak", "overflowWrap",
] as const;

/** Pixel position of a character inside a textarea, relative to the textarea's top-left. */
export function caretPosition(textarea: HTMLTextAreaElement, index: number): { top: number; left: number; height: number } {
  const mirror = document.createElement("div");
  const style = getComputedStyle(textarea);
  for (const prop of MIRRORED) mirror.style[prop] = style[prop];
  mirror.style.position = "absolute";
  mirror.style.visibility = "hidden";
  mirror.style.whiteSpace = "pre-wrap";
  mirror.style.overflowWrap = "break-word";
  mirror.textContent = textarea.value.slice(0, index);
  const marker = document.createElement("span");
  marker.textContent = textarea.value.slice(index) || ".";
  mirror.appendChild(marker);
  document.body.appendChild(mirror);
  const lineHeight = parseFloat(style.lineHeight) || 20;
  const pos = { top: marker.offsetTop - textarea.scrollTop, left: marker.offsetLeft, height: lineHeight };
  mirror.remove();
  return pos;
}
