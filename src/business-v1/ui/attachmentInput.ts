const MAX_HOST_PATH_LENGTH = 32_767;

type UnknownRecord = Record<string, unknown>;

export function extractClipboardImages(dataTransfer: DataTransfer | null): File[] {
  if (!dataTransfer) return [];

  const itemImages = Array.from(dataTransfer.items)
    .filter((item) => item.kind === "file" && item.type.toLowerCase().startsWith("image/"))
    .map((item) => item.getAsFile())
    .filter((file): file is File => file !== null);

  if (itemImages.length) return uniqueFiles(itemImages);

  return uniqueFiles(
    Array.from(dataTransfer.files).filter((file) => file.type.toLowerCase().startsWith("image/")),
  );
}

export function extractDroppedFiles(dataTransfer: DataTransfer | null): File[] {
  if (!dataTransfer) return [];
  return uniqueFiles(Array.from(dataTransfer.files));
}

export function hasFileDrop(dataTransfer: DataTransfer | null): boolean {
  if (!dataTransfer) return false;
  return Array.from(dataTransfer.types).some((type) => type.toLowerCase() === "files");
}

export function extractHostDropPaths(value: unknown): string[] {
  const candidates: unknown[] = [];
  const visited = new Set<unknown>();

  collectPathCandidates(value, candidates, visited, 0);

  return Array.from(
    new Set(
      candidates
        .filter((candidate): candidate is string => typeof candidate === "string")
        .map((candidate) => candidate.trim())
        .filter(isSafeAbsolutePath),
    ),
  );
}

function collectPathCandidates(
  value: unknown,
  candidates: unknown[],
  visited: Set<unknown>,
  depth: number,
): void {
  if (depth > 4 || value === null || value === undefined || visited.has(value)) return;

  if (Array.isArray(value)) {
    value.forEach((item) => {
      if (typeof item === "string") candidates.push(item);
      else collectPathCandidates(item, candidates, visited, depth + 1);
    });
    return;
  }

  if (typeof value !== "object") return;
  visited.add(value);

  const record = value as UnknownRecord;
  const pathValue = record.paths;
  if (Array.isArray(pathValue)) candidates.push(...pathValue);

  for (const key of ["payload", "detail", "event", "nativeEvent"]) {
    collectPathCandidates(record[key], candidates, visited, depth + 1);
  }
}

function isSafeAbsolutePath(value: string): boolean {
  if (!value || value.length > MAX_HOST_PATH_LENGTH || /[\0\r\n]/u.test(value)) return false;
  return /^(?:[a-zA-Z]:[\\/]|\\\\|\/)/u.test(value);
}

function uniqueFiles(files: File[]): File[] {
  const seen = new Set<string>();
  return files.filter((file) => {
    const key = `${file.name}\0${file.size}\0${file.lastModified}\0${file.type}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
