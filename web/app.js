import init, { compile_restrict_lang, lex_only, parse_only, wat_to_wasm } from './pkg/restrict_lang.js';

let wasmModule = null;
let activeTab = 'output';

const examples = {
    function: `// Values come before verbs.
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> () = {
    val result = (20, 22) add
    result |> print_int
}`,
    pipe: `// A pipe sends one value into a function.
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> () = {
    41 |> increment |> print_int
}`,
    record: `// Record fields use colons.
record Point {
    x: Int32
    y: Int32
}

fun make_origin: () -> Point = {
    Point { x: 0, y: 0 }
}

fun main: () -> () = {
    val origin = () make_origin
    origin.x |> print_int
}`,
    match: `// Built-in options can be matched.
fun choose: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(number) => { number }
        None => { 0 }
    }
}

fun main: () -> () = {
    Some(42) |> choose |> print_int
}`
};

async function initWasm() {
    try {
        wasmModule = await init();
        updateStatus('Compiler and runtime ready. Press Run or Ctrl/⌘ + Enter.', 'success');
        setOutput('output', 'Ready to run.');
        setOutput('wasm', 'Ready to compile.');
    } catch (error) {
        console.error('Failed to initialize WASM module:', error);
        updateStatus(`Failed to initialize compiler: ${error.message}`, 'error');
        setOutput('error', `Compiler initialization failed\n\n${error.message}`, true);
        showTab('error');
    }
}

function updateStatus(message, type = 'info') {
    const status = document.getElementById('compilerStatus');
    const statusText = document.getElementById('statusText');
    status.dataset.tone = type;
    statusText.textContent = message;
}

function setOutput(name, value, isError = false) {
    const panel = document.getElementById(name);
    const output = document.getElementById(`${name}Output`);
    output.textContent = value;
    panel.classList.toggle('is-error', isError);
}

function sourceValue() {
    return document.getElementById('sourceCode').value;
}

function ensureReady(action) {
    if (!wasmModule) {
        updateStatus(`Compiler is still loading; ${action} is not available yet.`, 'error');
        return false;
    }
    if (!sourceValue().trim()) {
        updateStatus(`Add source code before you ${action}.`, 'error');
        return false;
    }
    return true;
}

function errorMessage(error) {
    if (error instanceof Error) {
        return error.message;
    }
    return String(error);
}

function applyCompilationOutputs(result) {
    setOutput('wasm', result.output || 'Compilation succeeded without textual output.');
    setOutput('tokens', result.tokens || 'No token output was returned.');
    setOutput('ast', result.ast || 'No AST output was returned.');
    setOutput('error', 'No errors.');
}

function readWasiIovecs(memory, iovs, iovsLength) {
    const view = new DataView(memory.buffer);
    const chunks = [];
    let byteLength = 0;

    for (let index = 0; index < iovsLength; index += 1) {
        const entry = iovs + index * 8;
        const pointer = view.getUint32(entry, true);
        const length = view.getUint32(entry + 4, true);
        chunks.push(new Uint8Array(memory.buffer, pointer, length));
        byteLength += length;
    }

    return { chunks, byteLength };
}

async function executeWat(watSource) {
    const wasmBytes = wat_to_wasm(watSource).slice();
    const streams = {
        stdout: { decoder: new TextDecoder(), text: '' },
        stderr: { decoder: new TextDecoder(), text: '' }
    };
    const exitSignal = {};
    let exitCode = 0;
    let programInstance = null;

    const imports = {
        wasi_snapshot_preview1: {
            fd_write(fd, iovs, iovsLength, writtenPointer) {
                if (fd !== 1 && fd !== 2) {
                    return 8;
                }

                const memory = programInstance.exports.memory;
                const { chunks, byteLength } = readWasiIovecs(memory, iovs, iovsLength);
                const stream = fd === 1 ? streams.stdout : streams.stderr;
                for (const chunk of chunks) {
                    stream.text += stream.decoder.decode(chunk, { stream: true });
                }
                new DataView(memory.buffer).setUint32(writtenPointer, byteLength, true);
                return 0;
            },
            proc_exit(code) {
                exitCode = code;
                throw exitSignal;
            }
        }
    };

    const instantiated = await WebAssembly.instantiate(wasmBytes, imports);
    programInstance = instantiated.instance;
    const start = programInstance.exports._start;
    if (typeof start !== 'function') {
        throw new Error('No runnable entry point was generated. Add a zero-argument fun main: () -> ... function.');
    }

    try {
        start();
    } catch (error) {
        if (error !== exitSignal) {
            throw error;
        }
    }

    streams.stdout.text += streams.stdout.decoder.decode();
    streams.stderr.text += streams.stderr.decoder.decode();
    return { stdout: streams.stdout.text, stderr: streams.stderr.text, exitCode };
}

function formatProgramOutput(result) {
    const sections = [];
    if (result.stdout) {
        sections.push(result.stdout.trimEnd());
    }
    if (result.stderr) {
        sections.push(`[stderr]\n${result.stderr.trimEnd()}`);
    }
    if (result.exitCode !== 0) {
        sections.push(`[exit code ${result.exitCode}]`);
    }
    if (sections.length === 0) {
        return 'Program completed successfully.\n\n(no output — use print, println, print_int, or print_float to write a value)';
    }
    return sections.join('\n\n');
}

async function run() {
    if (!ensureReady('run')) {
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Compiling and running in this browser…', 'working');
        const result = compile_restrict_lang(sourceValue());

        if (!result.success) {
            setOutput('error', result.error || 'The compiler returned an unknown error.', true);
            if (result.tokens) {
                setOutput('tokens', result.tokens);
            }
            if (result.ast) {
                setOutput('ast', result.ast);
            }
            showTab('error');
            updateStatus('Run stopped with a compiler diagnostic.', 'error');
            return;
        }

        applyCompilationOutputs(result);
        const execution = await executeWat(result.output);
        setOutput('output', formatProgramOutput(execution), execution.exitCode !== 0);
        showTab('output');
        updateStatus(
            execution.exitCode === 0
                ? 'Program completed successfully.'
                : `Program exited with code ${execution.exitCode}.`,
            execution.exitCode === 0 ? 'success' : 'error'
        );
    } catch (error) {
        console.error('Program execution error:', error);
        setOutput('output', `Run error\n\n${errorMessage(error)}`, true);
        showTab('output');
        updateStatus('Program failed while running in the browser.', 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

async function compile() {
    if (!ensureReady('compile')) {
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Compiling the full pipeline…', 'working');
        const result = compile_restrict_lang(sourceValue());

        if (result.success) {
            applyCompilationOutputs(result);
            showTab('wasm');
            updateStatus('Compilation succeeded.', 'success');
        } else {
            setOutput('error', result.error || 'The compiler returned an unknown error.', true);
            if (result.tokens) {
                setOutput('tokens', result.tokens);
            }
            if (result.ast) {
                setOutput('ast', result.ast);
            }
            showTab('error');
            updateStatus('Compilation stopped with a diagnostic.', 'error');
        }
    } catch (error) {
        console.error('Compilation error:', error);
        setOutput('error', `Browser compiler error\n\n${errorMessage(error)}`, true);
        showTab('error');
        updateStatus('Compilation failed in the browser.', 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

async function lexOnly() {
    if (!ensureReady('tokenize')) {
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Tokenizing source…', 'working');
        const result = lex_only(sourceValue());

        if (result.success) {
            setOutput('tokens', result.tokens || 'No tokens were returned.');
            showTab('tokens');
            updateStatus('Tokenization succeeded.', 'success');
        } else {
            setOutput('error', result.error || 'Tokenization failed without a diagnostic.', true);
            showTab('error');
            updateStatus('Tokenization stopped with a diagnostic.', 'error');
        }
    } catch (error) {
        console.error('Tokenization error:', error);
        setOutput('error', `Browser tokenizer error\n\n${errorMessage(error)}`, true);
        showTab('error');
        updateStatus('Tokenization failed in the browser.', 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

async function parseOnly() {
    if (!ensureReady('parse')) {
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Parsing source…', 'working');
        const result = parse_only(sourceValue());

        if (result.success) {
            setOutput('ast', result.ast || 'No AST was returned.');
            showTab('ast');
            updateStatus('Parsing succeeded.', 'success');
        } else {
            setOutput('error', result.error || 'Parsing failed without a diagnostic.', true);
            showTab('error');
            updateStatus('Parsing stopped with a diagnostic.', 'error');
        }
    } catch (error) {
        console.error('Parsing error:', error);
        setOutput('error', `Browser parser error\n\n${errorMessage(error)}`, true);
        showTab('error');
        updateStatus('Parsing failed in the browser.', 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

function clearOutput() {
    setOutput('output', 'Ready to run.');
    setOutput('wasm', 'Ready to compile.');
    setOutput('tokens', 'No tokens yet.');
    setOutput('ast', 'No AST yet.');
    setOutput('error', 'No errors.');
    showTab('output');
}

function showTab(tabName) {
    const panels = document.querySelectorAll('.output-content');
    const buttons = document.querySelectorAll('.tab');

    panels.forEach((panel) => {
        panel.hidden = panel.id !== tabName;
    });
    buttons.forEach((button) => {
        const selected = button.dataset.tab === tabName;
        button.setAttribute('aria-selected', String(selected));
        button.tabIndex = selected ? 0 : -1;
    });

    activeTab = tabName;
    const labels = { output: 'Program output', wasm: 'Wasm output', tokens: 'Token output', ast: 'AST output', error: 'Error output' };
    document.getElementById('activeOutputLabel').textContent = labels[tabName];
}

function setButtonsDisabled(disabled) {
    for (const id of ['runBtn', 'compileBtn', 'lexBtn', 'parseBtn']) {
        document.getElementById(id).disabled = disabled;
    }
}

function loadExample(exampleName) {
    const example = examples[exampleName];
    if (!example) {
        return;
    }

    document.getElementById('sourceCode').value = example;
    document.getElementById('exampleSelect').value = exampleName;
    syncSourceHighlight();
    updateSourceStats();
    clearOutput();
    updateStatus('Example loaded. Ready to compile.', wasmModule ? 'success' : 'working');
}

function resetSource() {
    loadExample(document.getElementById('exampleSelect').value);
}

function escapeHtml(value) {
    return String(value)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function syncSourceHighlight() {
    const source = document.getElementById('sourceCode');
    const highlight = document.getElementById('sourceHighlight');
    if (!source || !highlight) {
        return;
    }

    const code = highlight.querySelector('code');
    const highlighter = window.RestrictHighlight;
    code.innerHTML = highlighter
        ? highlighter.highlightRestrict(source.value)
        : escapeHtml(source.value);

    highlight.scrollTop = source.scrollTop;
    highlight.scrollLeft = source.scrollLeft;
}

function updateSourceStats() {
    const value = sourceValue();
    const lines = value ? value.split('\n').length : 0;
    document.getElementById('sourceStats').textContent = `${lines} ${lines === 1 ? 'line' : 'lines'} · ${value.length} characters`;
}

async function copyText(value, successMessage) {
    try {
        await navigator.clipboard.writeText(value);
        updateStatus(successMessage, 'success');
    } catch (error) {
        console.error('Clipboard write failed:', error);
        updateStatus('Clipboard access was unavailable. Select and copy the text manually.', 'error');
    }
}

async function copyActiveOutput() {
    const value = document.getElementById(`${activeTab}Output`).textContent;
    await copyText(value, 'Active output copied to the clipboard.');
}

async function shareSource() {
    const url = new URL(window.location.href);
    url.searchParams.set('code', sourceValue());
    url.hash = '';
    await copyText(url.toString(), 'Share link copied. The source is encoded in the URL.');
}

function initializeSourceHighlighting() {
    const source = document.getElementById('sourceCode');
    source.addEventListener('input', syncSourceHighlight);
    source.addEventListener('input', updateSourceStats);
    source.addEventListener('scroll', syncSourceHighlight);
    syncSourceHighlight();
    updateSourceStats();
}

function initializeActions() {
    document.getElementById('runBtn').addEventListener('click', run);
    document.getElementById('compileBtn').addEventListener('click', compile);
    document.getElementById('lexBtn').addEventListener('click', lexOnly);
    document.getElementById('parseBtn').addEventListener('click', parseOnly);
    document.getElementById('resetBtn').addEventListener('click', resetSource);
    document.getElementById('shareBtn').addEventListener('click', shareSource);
    document.getElementById('copyOutputBtn').addEventListener('click', copyActiveOutput);
    document.getElementById('exampleSelect').addEventListener('change', (event) => loadExample(event.target.value));

    const tabButtons = Array.from(document.querySelectorAll('.tab'));
    tabButtons.forEach((button, index) => {
        button.addEventListener('click', () => showTab(button.dataset.tab));
        button.addEventListener('keydown', (event) => {
            if (!['ArrowLeft', 'ArrowRight'].includes(event.key)) {
                return;
            }
            event.preventDefault();
            const direction = event.key === 'ArrowRight' ? 1 : -1;
            const next = (index + direction + tabButtons.length) % tabButtons.length;
            showTab(tabButtons[next].dataset.tab);
            tabButtons[next].focus();
        });
    });

    document.getElementById('sourceCode').addEventListener('keydown', (event) => {
        if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault();
            run();
        }
    });
}

function loadInitialCodeFromQuery() {
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    if (code !== null) {
        document.getElementById('sourceCode').value = code;
        syncSourceHighlight();
        updateSourceStats();
        clearOutput();
    }
}

window.run = run;
window.compile = compile;
window.lexOnly = lexOnly;
window.parseOnly = parseOnly;
window.clearOutput = clearOutput;
window.showTab = showTab;
window.loadExample = loadExample;

document.addEventListener('DOMContentLoaded', () => {
    initializeSourceHighlighting();
    initializeActions();
    loadInitialCodeFromQuery();
    initWasm();
});
