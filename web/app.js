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
let editor = null;  // CodeMirror instance

// Initialize CodeMirror editor
function initEditor() {
    const textarea = document.getElementById('sourceCode');
    if (!textarea) return;

    editor = CodeMirror.fromTextArea(textarea, {
        mode: 'restrict',
        theme: 'restrict-dark',
        lineNumbers: true,
        matchBrackets: true,
        autoCloseBrackets: true,
        styleActiveLine: true,
        indentUnit: 4,
        tabSize: 4,
        indentWithTabs: false,
        lineWrapping: false,
        extraKeys: {
            'Ctrl-Enter': function() { compile(); },
            'Cmd-Enter': function() { compile(); },
            'Tab': function(cm) {
                if (cm.somethingSelected()) {
                    cm.indentSelection('add');
                } else {
                    cm.replaceSelection('    ', 'end');
                }
            }
        }
    });

    // Refresh editor layout after initialization
    setTimeout(() => editor.refresh(), 100);
}

// Get source code from editor
function getSourceCode() {
    return editor ? editor.getValue() : document.getElementById('sourceCode')?.value || '';
}

// Set source code in editor
function setSourceCode(code) {
    if (editor) {
        editor.setValue(code);
    } else {
        const textarea = document.getElementById('sourceCode');
        if (textarea) textarea.value = code;
    }
}

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

    const sourceCode = getSourceCode();
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

        // Collect console output
        let consoleOutput = [];
        let instance = null;  // Will be set after instantiation

        // WASI shim for browser execution
        const wasiShim = {
            // fd_write(fd, iovs, iovs_len, nwritten) - write to file descriptor
            fd_write: (fd, iovs, iovs_len, nwritten) => {
                // fd 1 = stdout, fd 2 = stderr
                const memory = instance.exports.memory;
                if (!memory) return 0;

                const view = new DataView(memory.buffer);
                let written = 0;
                const decoder = new TextDecoder();

                for (let i = 0; i < iovs_len; i++) {
                    const ptr = view.getUint32(iovs + i * 8, true);
                    const len = view.getUint32(iovs + i * 8 + 4, true);
                    const bytes = new Uint8Array(memory.buffer, ptr, len);
                    const text = decoder.decode(bytes);
                    consoleOutput.push(text);
                    written += len;
                }

                if (nwritten) {
                    view.setUint32(nwritten, written, true);
                }
                return 0; // Success
            },
            // Other WASI functions (stubs)
            fd_seek: () => 0,
            fd_close: () => 0,
            fd_read: () => 0,
            path_open: () => 8, // EBADF - file operations not supported in browser
            fd_prestat_get: () => 8, // EBADF
            fd_prestat_dir_name: () => 8,
            environ_sizes_get: (count_ptr, buf_size_ptr) => {
                const memory = instance.exports.memory;
                if (memory) {
                    const view = new DataView(memory.buffer);
                    view.setUint32(count_ptr, 0, true);
                    view.setUint32(buf_size_ptr, 0, true);
                }
                return 0;
            },
            environ_get: () => 0,
            args_sizes_get: (argc_ptr, argv_buf_size_ptr) => {
                const memory = instance.exports.memory;
                if (memory) {
                    const view = new DataView(memory.buffer);
                    view.setUint32(argc_ptr, 0, true);
                    view.setUint32(argv_buf_size_ptr, 0, true);
                }
                return 0;
            },
            args_get: () => 0,
            proc_exit: (code) => { throw new Error(`Process exited with code ${code}`); },
            clock_time_get: () => 0,
            random_get: (buf, len) => {
                const memory = instance.exports.memory;
                if (memory) {
                    const bytes = new Uint8Array(memory.buffer, buf, len);
                    crypto.getRandomValues(bytes);
                }
                return 0;
            },
        };

        const importObject = {
            wasi_snapshot_preview1: wasiShim,
            env: {},
        };

        instance = await WebAssembly.instantiate(module, importObject);

        // Try to call main function or _start
        let result;
        if (instance.exports._start) {
            instance.exports._start();
            result = 'completed';
        } else if (instance.exports.main) {
            result = instance.exports.main();
        } else {
            // List exported functions
            const exports = Object.keys(instance.exports).filter(k => typeof instance.exports[k] === 'function');
            document.getElementById('resultOutput').innerHTML =
                `<div class="info">No main/_start function. Exported functions: ${exports.join(', ') || 'none'}</div>`;
            return;
        }

        // Display result
        let output = '';
        if (consoleOutput.length > 0) {
            output += `<div class="console-output"><strong>Output:</strong><pre>${escapeHtml(consoleOutput.join(''))}</pre></div>`;
        }
        output += `<div class="success">Return value: <strong>${result}</strong></div>`;
        document.getElementById('resultOutput').innerHTML = output;

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

    const sourceCode = getSourceCode();
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

    const sourceCode = getSourceCode();
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

fun main = {
    "Hello, World!" |> println
}`,

        'fizzbuzz': `// FizzBuzz - Classic programming challenge

fun fizzbuzz: (n: Int) -> String = {
    n % 15 == 0 then { "FizzBuzz" } else {
        n % 3 == 0 then { "Fizz" } else {
            n % 5 == 0 then { "Buzz" } else {
                n int_to_string
            }
        }
    }
}

fun main = {
    mut val i = 1
    i <= 20 while {
        i fizzbuzz |> println
        i = i + 1
    }
}`,

        'affine': `// Affine Types - Each value can only be used once

fun main = {
    val message = "I can only be used once!"

    // This works - message is used exactly once
    message |> println

    // Try uncommenting to see the error:
    // message |> println  // Error: use of moved value
}`,

        'pipe': `// Pipe Operators & OSV Syntax
// Object-Subject-Verb order: "data function" instead of "function(data)"

fun double: (n: Int) -> Int = {
    n * 2
}

fun main = {
    // Traditional: double(double(5))
    // With pipes: 5 |> double |> double

    val result = 5 double double
    result int_to_string |> println

    // Or with explicit pipes:
    10 |> double |> int_to_string |> println
}`,

        'record': `// Records with Arena-based Memory

record Point { x: Int, y: Int }

fun main = {
    with Arena {
        val p = Point { x = 10, y = 20 }

        // Clone to create a copy
        val p2 = p.clone {}

        // Use the clone
        p2.x int_to_string |> println
    }
}`,

        'mutable': `// Mutable Variables

fun countdown: (n: Int) = {
    mut val i = n
    i > 0 while {
        i int_to_string |> println
        i = i - 1
    }
    "Liftoff!" |> println
}

fun main = {
    5 countdown
}`,

        'context': `// Context Binding - Implicit parameters via context

record Connection { id: Int }

// Define a context that provides a database connection
context Database {
    val conn: Connection
}

// Function requires Database context (like Reader monad)
fun query: (sql: String) -> String with Database = {
    sql  // In real code, would use conn from context
}

fun main = {
    with Arena {
        val conn = Connection { id = 1 }

        // Provide the Database context
        with Database { conn = conn } {
            "SELECT * FROM users" query |> println
        }
    }
}`,

        'compose': `// Scope Composition - Multiple contexts

record Logger { level: Int }
record Config { debug: Int }

context Logging { val logger: Logger }
context Configuration { val config: Config }

// Function requires BOTH contexts
fun log_with_config: () with Logging, Configuration = {
    "Logging with config" |> println
}

fun main = {
    with Arena {
        val log = Logger { level = 1 }
        val cfg = Config { debug = 1 }

        // Nested context scopes compose automatically
        with Logging { logger = log } {
            with Configuration { config = cfg } {
                log_with_config
            }
        }
    }
}`
    };

    const example = examples[exampleName];
    if (example) {
        setSourceCode(example);
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
document.addEventListener('DOMContentLoaded', () => {
    initEditor();
    initWasm();
});