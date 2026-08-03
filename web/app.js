import init, { compile_restrict_lang, lex_only, parse_only } from './pkg/restrict_lang.js';

let wasmModule = null;
let activeTab = 'wasm';

const examples = {
    function: `// Values come before verbs.
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    (20, 22) add
}`,
    pipe: `// A pipe sends one value into a function.
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> increment
}`,
    record: `// Record fields use colons.
record Point {
    x: Int32
    y: Int32
}

fun make_origin: () -> Point = {
    Point { x: 0, y: 0 }
}`,
    match: `// Built-in options can be matched.
fun choose: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(number) => { number }
        None => { 0 }
    }
}

fun main: () -> Int32 = {
    Some(42) |> choose
}`
};

async function initWasm() {
    try {
        wasmModule = await init();
        updateStatus('Compiler ready. Choose an action or press Ctrl/⌘ + Enter.', 'success');
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

async function compile() {
    if (!ensureReady('compile')) {
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Compiling the full pipeline…', 'working');
        const result = compile_restrict_lang(sourceValue());

        if (result.success) {
            setOutput('wasm', result.output || 'Compilation succeeded without textual output.');
            setOutput('tokens', result.tokens || 'No token output was returned.');
            setOutput('ast', result.ast || 'No AST output was returned.');
            setOutput('error', 'No errors.');
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
        setOutput('error', `Browser compiler error\n\n${error.message}`, true);
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
        setOutput('error', `Browser tokenizer error\n\n${error.message}`, true);
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
        setOutput('error', `Browser parser error\n\n${error.message}`, true);
        showTab('error');
        updateStatus('Parsing failed in the browser.', 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

function clearOutput() {
    setOutput('wasm', 'Ready to compile.');
    setOutput('tokens', 'No tokens yet.');
    setOutput('ast', 'No AST yet.');
    setOutput('error', 'No errors.');
    showTab('wasm');
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
    const labels = { wasm: 'Wasm output', tokens: 'Token output', ast: 'AST output', error: 'Error output' };
    document.getElementById('activeOutputLabel').textContent = labels[tabName];
}

function setButtonsDisabled(disabled) {
    for (const id of ['compileBtn', 'lexBtn', 'parseBtn']) {
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
            compile();
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
