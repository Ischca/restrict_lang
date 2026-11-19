import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const staticRoot = fileURLToPath(new URL("../.next/static/", import.meta.url));
const componentMarkers = ["demo:restrict-next-demo", "pricing01Quote"];

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map((entry) => {
      const path = `${directory}/${entry.name}`;
      return entry.isDirectory() ? collectFiles(path) : [path];
    }),
  );
  return nested.flat();
}

const leakedFiles = [];
for (const path of await collectFiles(staticRoot)) {
  const bytes = await readFile(path);
  const contents = bytes.toString("utf8");
  if (componentMarkers.some((marker) => contents.includes(marker))) {
    leakedFiles.push(path);
  }
}

assert.deepEqual(
  leakedFiles,
  [],
  `Restrict Component code leaked into browser assets:\n${leakedFiles.join("\n")}`,
);

console.log("Server-only boundary smoke passed");
