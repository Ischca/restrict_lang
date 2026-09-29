import type { Metadata } from "next";

import "./styles.css";

export const metadata: Metadata = {
  title: "Restrict × Next.js Component demo",
  description: "A Next.js host calling synchronous Restrict domain logic.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
