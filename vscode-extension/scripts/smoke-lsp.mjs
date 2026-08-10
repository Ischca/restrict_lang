import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const extensionDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(extensionDirectory, '..', '..');
const executable = process.platform === 'win32' ? 'restrict_lang.exe' : 'restrict_lang';
const serverCommand = process.argv[2] ?? path.join(repositoryRoot, 'target', 'debug', executable);

const child = spawn(serverCommand, ['--lsp'], {
    cwd: repositoryRoot,
    stdio: ['pipe', 'pipe', 'pipe']
});

let stderr = '';
let nextRequestId = 1;
let input = Buffer.alloc(0);
const pending = new Map();
let smokeDirectory;

child.stderr.setEncoding('utf8');
child.stderr.on('data', chunk => {
    stderr += chunk;
});

child.stdout.on('data', chunk => {
    input = Buffer.concat([input, chunk]);
    drainMessages();
});

child.on('error', error => {
    for (const { reject } of pending.values()) {
        reject(error);
    }
    pending.clear();
});

function drainMessages() {
    while (true) {
        const headerEnd = input.indexOf('\r\n\r\n');
        if (headerEnd < 0) {
            return;
        }

        const header = input.subarray(0, headerEnd).toString('ascii');
        const lengthMatch = header.match(/Content-Length:\s*(\d+)/i);
        assert(lengthMatch, `missing Content-Length header: ${header}`);
        const contentLength = Number(lengthMatch[1]);
        const bodyStart = headerEnd + 4;
        if (input.length < bodyStart + contentLength) {
            return;
        }

        const body = input.subarray(bodyStart, bodyStart + contentLength).toString('utf8');
        input = input.subarray(bodyStart + contentLength);
        const message = JSON.parse(body);
        if (message.id === undefined) {
            continue;
        }

        const waiter = pending.get(message.id);
        if (!waiter) {
            continue;
        }
        pending.delete(message.id);
        clearTimeout(waiter.timeout);
        if (message.error) {
            waiter.reject(new Error(JSON.stringify(message.error)));
        } else {
            waiter.resolve(message.result);
        }
    }
}

function write(message) {
    const body = JSON.stringify(message);
    child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

function notify(method, params = {}) {
    write({ jsonrpc: '2.0', method, params });
}

function request(method, params) {
    const id = nextRequestId++;
    const message = { jsonrpc: '2.0', id, method };
    if (params !== undefined) {
        message.params = params;
    }
    write(message);
    return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
            pending.delete(id);
            reject(new Error(`timed out waiting for ${method}; stderr: ${stderr}`));
        }, 10_000);
        pending.set(id, { resolve, reject, timeout });
    });
}

async function waitForExit() {
    if (child.exitCode !== null) {
        return child.exitCode;
    }
    return new Promise(resolve => child.once('exit', code => resolve(code)));
}

try {
    smokeDirectory = await mkdtemp(path.join(os.tmpdir(), 'restrict-lsp-smoke-'));
    const releasePath = path.join(smokeDirectory, 'release.rl');
    const mainPath = path.join(smokeDirectory, 'main.rl');
    const releaseSource = [
        'pub enum Status { Ready, Failed(String) }',
        'pub fun score: (value: Int32) -> Int32 = { value + 1 }',
        ''
    ].join('\n');
    const source = [
        'import release.{score, Status}',
        'fun main: () = {',
        '    val ready: Status = () Status::Ready',
        '    val result = 41 |> score;',
        '    result',
        '}',
        ''
    ].join('\n');
    await writeFile(releasePath, releaseSource);
    await writeFile(mainPath, source);

    const initialize = await request('initialize', {
        processId: process.pid,
        rootUri: pathToFileURL(smokeDirectory).toString(),
        capabilities: {}
    });
    assert.equal(initialize.serverInfo.name, 'Restrict Language Server');
    assert.equal(initialize.capabilities.hoverProvider, true);
    assert(initialize.capabilities.completionProvider);
    assert(initialize.capabilities.semanticTokensProvider);
    assert(initialize.capabilities.codeLensProvider);
    assert.equal(initialize.capabilities.inlayHintProvider, true);

    notify('initialized');
    const uri = pathToFileURL(await realpath(mainPath)).toString();
    const releaseUri = pathToFileURL(await realpath(releasePath)).toString();
    notify('textDocument/didOpen', {
        textDocument: {
            uri,
            languageId: 'restrict',
            version: 1,
            text: source
        }
    });

    const completion = await request('textDocument/completion', {
        textDocument: { uri },
        position: { line: 2, character: '    val ready: Status = () Status::'.length }
    });
    const completionItems = Array.isArray(completion) ? completion : completion.items;
    assert.deepEqual(
        completionItems.map(item => item.label),
        ['Status::Ready', 'Status::Failed']
    );

    const definition = await request('textDocument/definition', {
        textDocument: { uri },
        position: { line: 3, character: '    val result = 41 |> sc'.length }
    });
    assert.equal(definition.uri, releaseUri);
    assert.deepEqual(definition.range.start, { line: 1, character: 8 });

    const references = await request('textDocument/references', {
        textDocument: { uri },
        position: { line: 3, character: '    val result = 41 |> sc'.length },
        context: { includeDeclaration: true }
    });
    assert(references.some(location => location.uri === releaseUri));
    assert.equal(
        references.filter(location => location.uri === uri).length,
        2,
        JSON.stringify(references)
    );

    const hints = await request('textDocument/inlayHint', {
        textDocument: { uri },
        range: { start: { line: 0, character: 0 }, end: { line: 20, character: 0 } }
    });
    assert(hints.some(hint => hint.label === ': Int32'));
    assert(hints.some(hint => hint.label === '-> Int32'));

    notify('textDocument/didOpen', {
        textDocument: {
            uri: releaseUri,
            languageId: 'restrict',
            version: 1,
            text: releaseSource
        }
    });
    const lenses = await request('textDocument/codeLens', {
        textDocument: { uri: releaseUri }
    });
    const scoreLens = lenses.find(lens => lens.range.start.line === 1);
    assert.equal(scoreLens.command.title, '2 references');
    assert.equal(scoreLens.command.command, 'editor.action.showReferences');

    const semanticTokens = await request('textDocument/semanticTokens/full', {
        textDocument: { uri }
    });
    assert(semanticTokens.data.length > 0);

    const ast = await request('workspace/executeCommand', {
        command: 'restrict.lsp.showAST',
        arguments: [uri]
    });
    assert.equal(ast.success, true);
    assert.match(ast.ast, /main/);

    await request('shutdown');
    notify('exit');
    child.stdin.end();
    const exitCode = await waitForExit();
    assert.equal(exitCode, 0, stderr);
    await rm(smokeDirectory, { recursive: true, force: true });
    console.log('Restrict LSP smoke test passed.');
} catch (error) {
    child.kill();
    if (smokeDirectory) {
        await rm(smokeDirectory, { recursive: true, force: true });
    }
    console.error(error);
    if (stderr) {
        console.error(stderr);
    }
    process.exitCode = 1;
}
