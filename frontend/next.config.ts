import type { NextConfig } from "next";

// Static export: the Rust server embeds `out/` and serves it alongside /api.
// Every app URL is answered with the same shell (see crates/server/src/static_files.rs);
// routing happens on the client in lib/router.tsx.
const nextConfig: NextConfig = {
  output: "export",
  images: { unoptimized: true },
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
    },
  },
};

export default nextConfig;
