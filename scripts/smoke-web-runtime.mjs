import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

import init, { compile_restrict_lang, wat_to_wasm } from '../web/pkg/restrict_lang.js';

const compilerWasm = await readFile(new URL('../web/pkg/restrict_lang_bg.wasm', import.meta.url));
await init({ module_or_path: compilerWasm });

const source = `fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> () = {
    val result = (20, 22) add
    result |> print_int
}`;

const compilation = compile_restrict_lang(source);
assert.equal(compilation.success, true, compilation.error || 'playground smoke source should compile');

const outputChunks = [];
let programInstance = null;
const imports = {
    wasi_snapshot_preview1: {
        fd_write(fd, iovs, iovsLength, writtenPointer) {
            assert.equal(fd, 1, 'smoke program should only write to stdout');
            const memory = programInstance.exports.memory;
            const view = new DataView(memory.buffer);
            let byteLength = 0;

            for (let index = 0; index < iovsLength; index += 1) {
                const entry = iovs + index * 8;
                const pointer = view.getUint32(entry, true);
                const length = view.getUint32(entry + 4, true);
                outputChunks.push(new Uint8Array(memory.buffer, pointer, length).slice());
                byteLength += length;
            }

            new DataView(memory.buffer).setUint32(writtenPointer, byteLength, true);
            return 0;
        },
        proc_exit(code) {
            throw new Error(`smoke program exited unexpectedly with code ${code}`);
        }
    }
};

const instantiated = await WebAssembly.instantiate(wat_to_wasm(compilation.output), imports);
programInstance = instantiated.instance;
assert.equal(typeof programInstance.exports._start, 'function', 'program should export _start');
programInstance.exports._start();

const outputLength = outputChunks.reduce((total, chunk) => total + chunk.length, 0);
const outputBytes = new Uint8Array(outputLength);
let offset = 0;
for (const chunk of outputChunks) {
    outputBytes.set(chunk, offset);
    offset += chunk.length;
}

assert.equal(new TextDecoder().decode(outputBytes), '42\n');
console.log('web runtime smoke: 42');
