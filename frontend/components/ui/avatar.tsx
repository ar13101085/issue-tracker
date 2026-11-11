import { cn } from "@/lib/utils";

// Default avatar: a 5x5 mirrored pattern drawn from the login, like GitHub's identicons.
function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function Avatar({ login, size = 20, className }: { login: string; size?: number; className?: string }) {
  const h = hash(login.toLowerCase());
  const hue = h % 360;
  const cells = [];
  for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 3; col++) {
      if ((h >>> (row * 3 + col + 9)) & 1) {
        cells.push(<rect key={`${row}-${col}`} x={col + 0.5} y={row + 0.5} width={1} height={1} />);
        if (col < 2) cells.push(<rect key={`${row}-${4 - col}`} x={4 - col + 0.5} y={row + 0.5} width={1} height={1} />);
      }
    }
  }
  return (
    <svg
      viewBox="0 0 6 6"
      width={size}
      height={size}
      role="img"
      aria-label={`@${login}`}
      className={cn("shrink-0 rounded-full shadow-[0_0_0_1px_var(--avatar-borderColor)]", className)}
      style={{ background: "#f0f0f0" }}
      shapeRendering="crispEdges"
    >
      <g fill={`hsl(${hue} 55% 55%)`}>{cells}</g>
    </svg>
  );
}
