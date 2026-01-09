import init, {
    compile_restrict_lang,
    compile_with_diagnostics,
    get_inlay_hints,
    get_semantic_tokens,
    get_symbols,
    lex_only,
    parse_only
} from './pkg/restrict_lang.js';

let wasmModule = null;
let wabtModule = null;

// Initialize the WASM module
async function initWasm() {
    try {
        wasmModule = await init();
        console.log('Restrict Language WASM module initialized');

        // Initialize wabt for WAT->WASM conversion
        if (window.WabtModule) {
            wabtModule = await window.WabtModule();
            console.log('wabt module initialized');
        }

        updateStatus('Ready to compile! (Ctrl+Enter to run)', 'success');
    } catch (error) {
        console.error('Failed to initialize WASM module:', error);
        updateStatus('Failed to initialize compiler: ' + error.message, 'error');
    }
}

// Update status display
function updateStatus(message, type = 'info') {
    const statusEl = document.getElementById('statusMessage');
    if (statusEl) {
        statusEl.className = 'status-' + type;
        statusEl.textContent = message;
    }
    const wasmOutput = document.getElementById('wasmOutput');
    if (type === 'error' && wasmOutput) {
        wasmOutput.innerHTML = `<div class="error">${escapeHtml(message)}</div>`;
    }
}

// Escape HTML
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Compile the source code with rich diagnostics
async function compile() {
    if (!wasmModule) {
        updateStatus('Compiler not initialized yet. Please wait...', 'error');
        return;
    }

    const sourceCode = document.getElementById('sourceCode').value;
    if (!sourceCode.trim()) {
        updateStatus('Please enter some source code to compile.', 'error');
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Compiling...', 'info');

        // Use the new rich diagnostics API
        const result = compile_with_diagnostics(sourceCode);

        if (result.success) {
            // Display WASM output
            document.getElementById('wasmOutput').innerHTML =
                `<div class="success">Compilation successful!</div><pre>${escapeHtml(result.output || 'No output generated')}</pre>`;

            // Display AST if available
            if (result.ast) {
                document.getElementById('astOutput').innerHTML =
                    `<pre>${escapeHtml(result.ast)}</pre>`;
            }

            // Clear errors
            document.getElementById('errorOutput').innerHTML =
                `<div class="success">No errors!</div>`;

            // Execute the WASM
            if (wabtModule && result.output) {
                await executeWasm(result.output);
            }

            updateStatus('Compilation successful!', 'success');
        } else {
            // Display rich errors
            displayRichErrors(result.errors, sourceCode);

            // Display partial results if available
            if (result.ast) {
                document.getElementById('astOutput').innerHTML =
                    `<pre>${escapeHtml(result.ast)}</pre>`;
            }

            document.getElementById('wasmOutput').innerHTML =
                `<div class="error">Compilation failed with ${result.errors.length} error(s)</div>`;

            // Show error tab
            showTab('error');
            updateStatus(`${result.errors.length} error(s)`, 'error');
        }

        // Update inlay hints
        updateInlayHints(sourceCode);

    } catch (error) {
        console.error('Compilation error:', error);
        updateStatus('Compilation failed: ' + error.message, 'error');
        document.getElementById('errorOutput').innerHTML =
            `<div class="error">JavaScript error: ${escapeHtml(error.message)}</div>`;
        showTab('error');
    } finally {
        setButtonsDisabled(false);
    }
}

// Display rich errors with line numbers and help
function displayRichErrors(errors, sourceCode) {
    const lines = sourceCode.split('\n');
    let html = '';

    for (const err of errors) {
        const lineNum = err.line + 1;
        const colNum = err.column + 1;
        const sourceLine = lines[err.line] || '';

        html += `<div class="error-item">
            <div class="error-header">
                <span class="error-code">${err.code ? `[${err.code}]` : ''}</span>
                <span class="error-severity ${err.severity}">${err.severity}</span>
            </div>
            <div class="error-location">Line ${lineNum}, Column ${colNum}</div>
            <div class="error-message">${escapeHtml(err.message)}</div>
            <div class="error-source">
                <span class="line-num">${lineNum}</span><span class="source-line">${escapeHtml(sourceLine)}</span>
                <span class="line-num"></span><span class="error-pointer">${' '.repeat(err.column)}${'~'.repeat(Math.max(1, err.end_column - err.column))}</span>
            </div>
            ${err.notes.map(n => `<div class="error-note">note: ${escapeHtml(n)}</div>`).join('')}
            ${err.help.map(h => `<div class="error-help">help: ${escapeHtml(h)}</div>`).join('')}
        </div>`;
    }

    document.getElementById('errorOutput').innerHTML = html || '<div class="success">No errors!</div>';
}

// Execute WAT code
async function executeWasm(watCode) {
    if (!wabtModule) {
        document.getElementById('resultOutput').innerHTML =
            '<div class="warning">wabt not loaded - cannot execute</div>';
        return;
    }

    try {
        const wasmModule = wabtModule.parseWat('program.wat', watCode);
        const binary = wasmModule.toBinary({});

        const module = await WebAssembly.compile(binary.buffer);
        const instance = await WebAssembly.instantiate(module, {});

        // Try to call main function
        if (instance.exports.main) {
            const result = instance.exports.main();
            document.getElementById('resultOutput').innerHTML =
                `<div class="success">Execution Result: <strong>${result}</strong></div>`;
        } else {
            // List exported functions
            const exports = Object.keys(instance.exports);
            document.getElementById('resultOutput').innerHTML =
                `<div class="info">No main function. Exports: ${exports.join(', ')}</div>`;
        }
    } catch (error) {
        document.getElementById('resultOutput').innerHTML =
            `<div class="error">Execution error: ${escapeHtml(error.message)}</div>`;
    }
}

// Update inlay hints display
function updateInlayHints(sourceCode) {
    try {
        const hints = get_inlay_hints(sourceCode);
        let html = '';

        if (hints && hints.length > 0) {
            for (const hint of hints) {
                html += `<div class="hint-item">
                    <span class="hint-location">Line ${hint.line + 1}, Col ${hint.column + 1}</span>
                    <span class="hint-label hint-${hint.kind}">${escapeHtml(hint.label)}</span>
                    ${hint.tooltip ? `<span class="hint-tooltip">${escapeHtml(hint.tooltip)}</span>` : ''}
                </div>`;
            }
        } else {
            html = '<div class="info">No type hints available</div>';
        }

        const hintsOutput = document.getElementById('hintsOutput');
        if (hintsOutput) {
            hintsOutput.innerHTML = html;
        }
    } catch (e) {
        console.error('Failed to get inlay hints:', e);
    }
}

// Lex only (outputs to AST tab)
async function lexOnly() {
    if (!wasmModule) {
        updateStatus('Compiler not initialized yet. Please wait...', 'error');
        return;
    }

    const sourceCode = document.getElementById('sourceCode').value;
    if (!sourceCode.trim()) {
        updateStatus('Please enter some source code to tokenize.', 'error');
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Tokenizing...', 'info');

        const result = lex_only(sourceCode);

        if (result.success) {
            document.getElementById('astOutput').innerHTML =
                `<h4>Tokens:</h4><pre>${escapeHtml(result.tokens || 'No tokens generated')}</pre>`;
            showTab('ast');
            updateStatus('Tokenization successful!', 'success');
        } else {
            document.getElementById('errorOutput').innerHTML =
                `<pre class="error">${escapeHtml(result.error || 'Unknown error occurred')}</pre>`;
            showTab('error');
            updateStatus('Tokenization failed', 'error');
        }
    } catch (error) {
        console.error('Tokenization error:', error);
        updateStatus('Tokenization failed: ' + error.message, 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

// Parse only
async function parseOnly() {
    if (!wasmModule) {
        updateStatus('Compiler not initialized yet. Please wait...', 'error');
        return;
    }

    const sourceCode = document.getElementById('sourceCode').value;
    if (!sourceCode.trim()) {
        updateStatus('Please enter some source code to parse.', 'error');
        return;
    }

    try {
        setButtonsDisabled(true);
        updateStatus('Parsing...', 'info');

        const result = parse_only(sourceCode);

        if (result.success) {
            document.getElementById('astOutput').innerHTML =
                `<pre>${escapeHtml(result.ast || 'No AST generated')}</pre>`;
            showTab('ast');
            updateStatus('Parsing successful!', 'success');
        } else {
            document.getElementById('errorOutput').innerHTML =
                `<pre class="error">${escapeHtml(result.error || 'Unknown error occurred')}</pre>`;
            showTab('error');
            updateStatus('Parsing failed', 'error');
        }
    } catch (error) {
        console.error('Parsing error:', error);
        updateStatus('Parsing failed: ' + error.message, 'error');
    } finally {
        setButtonsDisabled(false);
    }
}

// Clear output
function clearOutput() {
    document.getElementById('wasmOutput').innerHTML = 'Ready to compile...';
    document.getElementById('astOutput').innerHTML = 'No AST yet...';
    document.getElementById('errorOutput').innerHTML = 'No errors...';
    document.getElementById('hintsOutput').innerHTML = 'No type hints yet...';
    document.getElementById('resultOutput').innerHTML = '<div class="info">Click "Run" to execute...</div>';
    showTab('result');
}

// Show tab
function showTab(tabName) {
    // Hide all tabs
    const tabs = document.querySelectorAll('.output-content');
    tabs.forEach(tab => tab.style.display = 'none');

    // Remove active class from all tab buttons
    const tabButtons = document.querySelectorAll('.tab');
    tabButtons.forEach(button => button.classList.remove('active'));

    // Show selected tab
    document.getElementById(tabName).style.display = 'block';

    // Add active class to the corresponding tab button
    const tabMap = { 'result': 0, 'wasm': 1, 'error': 2, 'ast': 3, 'hints': 4 };
    if (tabMap[tabName] !== undefined && tabButtons[tabMap[tabName]]) {
        tabButtons[tabMap[tabName]].classList.add('active');
    }
}

// Set buttons disabled state
function setButtonsDisabled(disabled) {
    const compileBtn = document.getElementById('compileBtn');
    if (compileBtn) compileBtn.disabled = disabled;
}

// Load example
function loadExample(exampleName) {
    const examples = {
        'hello': `// Hello World in Restrict Language

fun main: () -> Int = {
    val answer = 42;
    answer
}`,

        'affine': `// Affine Types Demo
// Each value can only be used once!

fun main: () -> Int = {
    val x = 100;

    // x can only be used once
    // Try uncommenting the next lines to see the error:
    // val first = x;
    // val second = x;  // Error: use of moved value

    x
}`,

        'function': `// Function Definition

fun add: (a: Int, b: Int) -> Int = {
    42  // placeholder - full arithmetic coming soon
}

fun main: () -> Int = {
    val result = 10;
    result
}`,

        'record': `// Record Types

record Point = {
    x: Int,
    y: Int
}

fun main: () -> Int = {
    42
}`,

        'pipe': `// Pipe Operators (OSV Syntax)
// Object-Subject-Verb order

fun main: () -> Int = {
    val x = 42;
    x
}`
    };

    const example = examples[exampleName];
    if (example) {
        document.getElementById('sourceCode').value = example;
        clearOutput();
    }
}

// Make functions available globally
window.compile = compile;
window.lexOnly = lexOnly;
window.parseOnly = parseOnly;
window.clearOutput = clearOutput;
window.showTab = showTab;
window.loadExample = loadExample;

// Initialize when the page loads
document.addEventListener('DOMContentLoaded', initWasm);