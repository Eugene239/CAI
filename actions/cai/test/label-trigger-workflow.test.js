import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import test from "node:test";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "../../..");

test("the cai label workflow is issue-label-only and fails closed on initiator access", () => {
  const workflow = readFileSync(
    join(repositoryRoot, ".github/workflows/cai-on-label.yml"),
    "utf8",
  );

  assert.match(workflow, /issues:\n\s+types: \[labeled\]/);
  assert.match(workflow, /contents: read/);
  assert.match(workflow, /issues: read/);
  assert.match(workflow, /github\.event\.label\.name == 'cai'/);
  assert.match(
    workflow,
    /collaborators\/\$\{\{ github\.event\.sender\.login \}\}\/permission/,
  );
  assert.match(workflow, /write\|maintain\|admin/);
  assert.match(workflow, /uses: \.\/actions\/cai/);
  assert.doesNotMatch(workflow, /pull-requests: write/);
  assert.doesNotMatch(workflow, /contents: write/);
});
