import init, { compile_restrict_lang, lex_only, parse_only } from './pkg/restrict_lang.js';

let wasmModule = null;

// Initialize the WASM module
async function initWasm() {
    try {
        wasmModule = await init();
        console.log('WASM module initialized successfully');
        updateStatus('Ready to compile!', 'success');
    } catch (error) {
        console.error('Failed to initialize WASM module:', error);
        updateStatus('Failed to initialize compiler: ' + error.message, 'error');
    }
}

// Update status display
function updateStatus(message, type = 'info') {
    const wasmOutput = document.getElementById('wasmOutput');
    if (type === 'error') {
        wasmOutput.innerHTML = `<div class="error">${escapeHtml(message)}</div>`;
    } else if (type === 'success') {
        wasmOutput.innerHTML = `<div class="success">${escapeHtml(message)}</div>`;
    } else {
        wasmOutput.textContent = message;
    }
}

// Compile the source code
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
        
        const result = compile_restrict_lang(sourceCode);
        
        if (result.success) {
            // Display WASM output
            document.getElementById('wasmOutput').innerHTML = 
                `<div class="success">Compilation successful!</div><pre>${escapeHtml(result.output || 'No output generated')}</pre>`;
            
            // Display tokens if available
            if (result.tokens) {
                document.getElementById('tokensOutput').innerHTML = 
                    `<pre>${escapeHtml(result.tokens)}</pre>`;
            }
            
            // Display AST if available
            if (result.ast) {
                document.getElementById('astOutput').innerHTML = 
                    `<pre>${escapeHtml(result.ast)}</pre>`;
            }
            
            // Clear errors
            document.getElementById('errorOutput').innerHTML = 
                `<pre>No errors!</pre>`;
                
        } else {
            // Display error
            document.getElementById('errorOutput').innerHTML = 
                `<pre class="error">${escapeHtml(result.error || 'Unknown error occurred')}</pre>`;
            
            // Display partial results if available
            if (result.tokens) {
                document.getElementById('tokensOutput').innerHTML = 
                    `<pre>${escapeHtml(result.tokens)}</pre>`;
            }
            
            if (result.ast) {
                document.getElementById('astOutput').innerHTML = 
                    `<pre>${escapeHtml(result.ast)}</pre>`;
            }
            
            // Show error tab
            showTab('error');
        }
    } catch (error) {
        console.error('Compilation error:', error);
        updateStatus('Compilation failed: ' + error.message, 'error');
        document.getElementById('errorOutput').innerHTML = 
            `<pre class="error">JavaScript error: ${escapeHtml(error.message)}</pre>`;
        showTab('error');
    } finally {
        setButtonsDisabled(false);
    }
}

// Lex only
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
            document.getElementById('tokensOutput').innerHTML = 
                `<pre>${escapeHtml(result.tokens || 'No tokens generated')}</pre>`;
            showTab('tokens');
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
    document.getElementById('tokensOutput').innerHTML = 'No tokens yet...';
    document.getElementById('astOutput').innerHTML = 'No AST yet...';
    document.getElementById('errorOutput').innerHTML = 'No errors...';
    showTab('wasm');
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
    
    // Add active class to selected tab button
    const activeButton = document.querySelector(`.tab[data-tab="${tabName}"]`);
    if (activeButton) {
        activeButton.classList.add('active');
    }
}

// Set buttons disabled state
function setButtonsDisabled(disabled) {
    document.getElementById('compileBtn').disabled = disabled;
    document.getElementById('lexBtn').disabled = disabled;
    document.getElementById('parseBtn').disabled = disabled;
}

// Load example
function loadExample(exampleName) {
    const examples = {
        'hello': `// Hello World example
fun main: () -> Int32 = {
    42
}`,
        
        'function': `// Function definition example
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    val result = (2, 3) add
    result
}`,
        
        'pipe': `// Pipe operations example
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    42 |> increment
}`,
        
        'record': `// Record type example
record Point {
    x: Int32
    y: Int32
}

fun make_origin: () -> Point = {
    Point { x: 0, y: 0 }
}`
    };
    
    const example = examples[exampleName];
    if (example) {
        document.getElementById('sourceCode').value = example;
        syncSourceHighlight();
        clearOutput();
    }
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

function initializeSourceHighlighting() {
    const source = document.getElementById('sourceCode');
    if (!source) {
        return;
    }

    source.addEventListener('input', syncSourceHighlight);
    source.addEventListener('scroll', syncSourceHighlight);
    syncSourceHighlight();
}

// Make functions available globally
window.compile = compile;
window.lexOnly = lexOnly;
window.parseOnly = parseOnly;
window.clearOutput = clearOutput;
window.showTab = showTab;
window.loadExample = loadExample;

function loadInitialCodeFromQuery() {
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    if (code !== null) {
        document.getElementById('sourceCode').value = code;
        syncSourceHighlight();
        clearOutput();
    }
}

// Initialize when the page loads
document.addEventListener('DOMContentLoaded', () => {
    initializeSourceHighlighting();
    loadInitialCodeFromQuery();
    initWasm();
});
