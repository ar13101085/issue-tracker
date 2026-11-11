import type { Metadata, Viewport } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Issue Tracker",
  description: "Self-hosted issue tracker",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

// Runs before first paint: turns the saved preference ("light" | "dark" | "auto")
// into an explicit data-color-mode, so CSS never has to guess and nothing flashes.
const themeScript = `(function(){try{var d=document.documentElement,m=window.matchMedia('(prefers-color-scheme: dark)');function a(){var p=localStorage.getItem('color-mode')||'auto';d.setAttribute('data-color-mode',p==='auto'?(m.matches?'dark':'light'):p)}a();m.addEventListener('change',a);window.addEventListener('storage',a);window.__applyColorMode=a}catch(e){}})()`;

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" data-color-mode="light" data-light-theme="light" data-dark-theme="dark" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
      </head>
      <body className="min-h-screen antialiased">{children}</body>
    </html>
  );
}
