import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Link } from "@/lib/router";
import { cn } from "@/lib/utils";

// Primer Button. Colors come from the --button-* tokens so both themes match GitHub.
const VARIANTS = {
  default:
    "border-[var(--button-default-borderColor-rest)] bg-[var(--button-default-bgColor-rest)] text-[var(--button-default-fgColor-rest)] shadow-[var(--button-default-shadow-resting)] hover:bg-[var(--button-default-bgColor-hover)] active:bg-[var(--button-default-bgColor-active)]",
  primary:
    "border-[var(--button-primary-borderColor-rest)] bg-[var(--button-primary-bgColor-rest)] text-[var(--button-primary-fgColor-rest)] hover:bg-[var(--button-primary-bgColor-hover)] active:bg-[var(--button-primary-bgColor-active)]",
  danger:
    "border-[var(--button-danger-borderColor-rest)] bg-[var(--button-danger-bgColor-rest)] text-[var(--button-danger-fgColor-rest)] shadow-[var(--button-default-shadow-resting)] hover:border-[var(--button-danger-borderColor-hover)] hover:bg-[var(--button-danger-bgColor-hover)] hover:text-[var(--button-danger-fgColor-hover)]",
  invisible:
    "border-transparent bg-transparent text-[var(--button-invisible-fgColor-rest)] hover:bg-[var(--button-invisible-bgColor-hover)]",
} as const;

const SIZES = {
  sm: "h-7 px-2 text-xs",
  md: "h-8 px-3 text-sm",
  lg: "h-10 px-4 text-sm",
} as const;

type Props = {
  variant?: keyof typeof VARIANTS;
  size?: keyof typeof SIZES;
  block?: boolean;
  href?: string;
  leading?: ReactNode;
  trailing?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

export function buttonClass(variant: keyof typeof VARIANTS = "default", size: keyof typeof SIZES = "md", block = false) {
  return cn(
    "inline-flex shrink-0 cursor-pointer items-center justify-center gap-2 whitespace-nowrap rounded-md border font-medium select-none",
    "disabled:cursor-not-allowed disabled:opacity-60 aria-disabled:pointer-events-none aria-disabled:opacity-60",
    VARIANTS[variant],
    SIZES[size],
    block && "w-full",
  );
}

export function Button({ variant, size, block, href, leading, trailing, className, children, ...rest }: Props) {
  const cls = cn(buttonClass(variant, size, block), className);
  const content = (
    <>
      {leading && <span className={variant === "primary" ? "" : "text-fg-muted"}>{leading}</span>}
      {children}
      {trailing && <span className={variant === "primary" ? "" : "text-fg-muted"}>{trailing}</span>}
    </>
  );
  if (href) {
    return (
      <Link href={href} className={cls}>
        {content}
      </Link>
    );
  }
  return (
    <button type="button" className={cls} {...rest}>
      {content}
    </button>
  );
}
