#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const config = JSON.parse(
  readFileSync(join(root, "tauri", "tauri.windows.conf.json"), "utf8"),
);
const extensions = new Set(
  config.bundle.fileAssociations.flatMap((association) => association.ext),
);
assert.deepEqual(extensions, new Set(["hprof", "bin"]));
assert.ok(
  config.bundle.fileAssociations.every((association) => association.description),
  "every file association must describe the registered heap type",
);

const script = readFileSync(
  join(root, "scripts", "windows", "register-file-associations.ps1"),
  "utf8",
);
assert.match(script, /Registry\]::CurrentUser/);
assert.match(script, /Software\\Classes/);
assert.match(script, /OpenWithProgids/);
assert.match(script, /"\.hprof"/);
assert.match(script, /"\.bin"/);
assert.match(script, /--open `"%1`"/);
assert.doesNotMatch(script, /HKEY_CLASSES_ROOT/);

const portablePackager = readFileSync(
  join(root, "scripts", "release", "package_windows_portable.ps1"),
  "utf8",
);
assert.match(portablePackager, /register-file-associations\.ps1/);
assert.match(portablePackager, /Open With/i);

const main = readFileSync(join(root, "tauri", "src", "main.rs"), "utf8");
assert.match(main, /"File"/);
assert.match(main, /"Open Heap\.\.\."/);
assert.match(main, /"mnemosyne:\/\/open-heap-requested"/);

console.log("M31.A Windows OS integration contracts: pass");
