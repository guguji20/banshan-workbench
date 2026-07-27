import { createHash } from "node:crypto";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, posix, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const skillRoot = resolve(repoRoot, "src-tauri", "resources", "business-skills");
const bundlePath = resolve(skillRoot, "bundle.json");
const textExtensions = new Set([".json", ".md"]);

async function listManagedFiles(directory, prefix = "") {
  const entries = await readdir(directory, { withFileTypes: true });
  const paths = [];
  for (const entry of entries) {
    const relativePath = posix.join(prefix, entry.name);
    const absolutePath = resolve(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`symbolic links are not allowed: ${relativePath}`);
    if (entry.isDirectory()) {
      paths.push(...await listManagedFiles(absolutePath, relativePath));
    } else if (entry.isFile() && relativePath !== "bundle.json") {
      paths.push(relativePath);
    }
  }
  return paths.sort();
}

function assertManagedPath(relativePath) {
  const absolutePath = resolve(skillRoot, ...relativePath.split("/"));
  const rel = relative(skillRoot, absolutePath);
  if (rel === "" || rel === ".." || rel.startsWith(`..${sep}`)) {
    throw new Error(`business skill path escapes bundle root: ${relativePath}`);
  }
  return absolutePath;
}

function extensionOf(relativePath) {
  const index = relativePath.lastIndexOf(".");
  return index === -1 ? "" : relativePath.slice(index).toLowerCase();
}

const bundle = JSON.parse(await readFile(bundlePath, "utf8"));
const files = [];
for (const relativePath of await listManagedFiles(skillRoot)) {
  const absolutePath = assertManagedPath(relativePath);
  let contents = await readFile(absolutePath);
  if (textExtensions.has(extensionOf(relativePath))) {
    const normalized = contents.toString("utf8").replace(/\r\n?/g, "\n");
    contents = Buffer.from(normalized, "utf8");
    await writeFile(absolutePath, contents);
  }
  files.push({
    path: relativePath,
    bytes: contents.byteLength,
    sha256: createHash("sha256").update(contents).digest("hex"),
  });
}

bundle.files = files;
await writeFile(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`, "utf8");
console.log(`updated business skill bundle: ${files.length} files normalized and hashed`);
