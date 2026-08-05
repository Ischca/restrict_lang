import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, '..');
const samplesDir = resolve(root, 'samples/playground');
const manifestPath = resolve(samplesDir, 'manifest.json');
const outputPath = resolve(root, 'web/examples.js');
const checkOnly = process.argv.includes('--check');

const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
assert.ok(Array.isArray(manifest.groups) && manifest.groups.length > 0, 'playground manifest needs groups');
assert.ok(Array.isArray(manifest.samples) && manifest.samples.length > 0, 'playground manifest needs samples');

const groupIds = new Set();
for (const group of manifest.groups) {
    assert.equal(typeof group.id, 'string', 'every playground group needs an id');
    assert.equal(typeof group.title, 'string', `playground group ${group.id} needs a title`);
    assert.ok(!groupIds.has(group.id), `duplicate playground group id: ${group.id}`);
    groupIds.add(group.id);
}

const sampleIds = new Set();
const examples = [];
for (const sample of manifest.samples) {
    for (const field of ['id', 'title', 'group', 'description', 'file', 'kind']) {
        assert.equal(typeof sample[field], 'string', `playground sample needs ${field}`);
    }
    assert.ok(!sampleIds.has(sample.id), `duplicate playground sample id: ${sample.id}`);
    assert.ok(groupIds.has(sample.group), `unknown playground group ${sample.group}`);
    assert.ok(['run', 'diagnostic'].includes(sample.kind), `unknown playground sample kind ${sample.kind}`);

    if (sample.kind === 'run') {
        assert.equal(typeof sample.expectedOutput, 'string', `${sample.id} needs expectedOutput`);
    } else {
        assert.equal(typeof sample.expectedDiagnostic, 'string', `${sample.id} needs expectedDiagnostic`);
    }

    const source = await readFile(resolve(samplesDir, sample.file), 'utf8');
    sampleIds.add(sample.id);
    examples.push({ ...sample, source });
}

const generated = `// Auto-generated from samples/playground/manifest.json.\n// Run: bash scripts/sync_samples.sh\n\nexport const exampleGroups = ${JSON.stringify(manifest.groups, null, 4)};\n\nexport const examples = ${JSON.stringify(examples, null, 4)};\n\nexport const examplesById = Object.fromEntries(examples.map((example) => [example.id, example]));\n`;

if (checkOnly) {
    const current = await readFile(outputPath, 'utf8');
    assert.equal(current, generated, 'web/examples.js is out of date; run bash scripts/sync_samples.sh');
    console.log('web/examples.js is up to date.');
} else {
    await writeFile(outputPath, generated);
    console.log(`Generated web/examples.js from ${examples.length} playground samples.`);
}
