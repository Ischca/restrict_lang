// Restrict Language Documentation JavaScript

// Language switching functionality
document.addEventListener('DOMContentLoaded', function() {
    // Add language selector
    const languageSelector = createLanguageSelector();
    if (languageSelector) {
        document.querySelector('.menu-title').appendChild(languageSelector);
    }
    
    // Initialize code highlighting for OSV syntax
    initializeOSVHighlighting();
    
    // Add interactive examples
    initializeInteractiveExamples();
    
    // Setup tabs for multilingual examples
    setupExampleTabs();
});

function createLanguageSelector() {
    const currentPath = window.location.pathname;
    const isJapanese = currentPath.includes('/ja/');
    
    const selector = document.createElement('div');
    selector.className = 'language-selector';
    selector.innerHTML = `
        <select onchange="switchLanguage(this.value)">
            <option value="en" ${!isJapanese ? 'selected' : ''}>English</option>
            <option value="ja" ${isJapanese ? 'selected' : ''}>日本語</option>
        </select>
    `;
    
    return selector;
}

function switchLanguage(lang) {
    const currentPath = window.location.pathname;
    let newPath;
    
    if (lang === 'ja') {
        newPath = currentPath.replace(/\/en\//, '/ja/');
        if (!newPath.includes('/ja/')) {
            newPath = currentPath.replace(/\/docs\//, '/docs/ja/');
        }
    } else {
        newPath = currentPath.replace(/\/ja\//, '/en/');
        if (!newPath.includes('/en/')) {
            newPath = currentPath.replace(/\/docs\//, '/docs/en/');
        }
    }
    
    window.location.pathname = newPath;
}

function initializeOSVHighlighting() {
    // Find all Restrict code blocks
    document.querySelectorAll('pre code.language-restrict').forEach(block => {
        // Highlight pipe operators
        block.innerHTML = block.innerHTML.replace(/\|&gt;/g, '<span class="pipe">|&gt;</span>');
        block.innerHTML = block.innerHTML.replace(/\|&gt;&gt;/g, '<span class="pipe">|&gt;&gt;</span>');
        
        // Add OSV indicator for complex expressions
        const lines = block.textContent.split('\n');
        lines.forEach((line, index) => {
            if (line.includes('|>') && !line.trim().startsWith('//')) {
                // This line uses OSV syntax
                const lineElement = block.querySelector(`span:nth-child(${index + 1})`);
                if (lineElement) {
                    lineElement.classList.add('osv-line');
                }
            }
        });
    });
}

function initializeInteractiveExamples() {
    // Add "Try it" buttons to code examples
    document.querySelectorAll('pre code.language-restrict').forEach((block, index) => {
        const pre = block.parentElement;
        const code = block.textContent;
        
        // Skip if it's not a complete example
        if (!code.includes('fn main') && !code.includes('fun main')) {
            return;
        }
        
        const button = document.createElement('button');
        button.className = 'try-button';
        button.textContent = 'Try in Playground';
        button.onclick = () => openInPlayground(code);
        
        pre.style.position = 'relative';
        pre.appendChild(button);
    });
}

function openInPlayground(code) {
    // Open Restrict Language playground with the code
    const playgroundUrl = 'https://play.restrict-lang.org';
    const encoded = encodeURIComponent(code);
    window.open(`${playgroundUrl}?code=${encoded}`, '_blank');
}

function setupExampleTabs() {
    document.querySelectorAll('.example-tabs').forEach(tabContainer => {
        const buttons = tabContainer.querySelectorAll('.tab-button');
        const panes = tabContainer.querySelectorAll('.tab-pane');
        
        buttons.forEach((button, index) => {
            button.addEventListener('click', () => {
                // Remove active class from all
                buttons.forEach(btn => btn.classList.remove('active'));
                panes.forEach(pane => pane.classList.remove('active'));
                
                // Add active class to clicked
                button.classList.add('active');
                panes[index].classList.add('active');
            });
        });
        
        // Activate first tab by default
        if (buttons.length > 0) {
            buttons[0].click();
        }
    });
}

// Add copy button to code blocks
document.querySelectorAll('pre code').forEach(block => {
    const pre = block.parentElement;
    const button = document.createElement('button');
    button.className = 'copy-button';
    button.textContent = 'Copy';
    
    button.addEventListener('click', () => {
        navigator.clipboard.writeText(block.textContent).then(() => {
            button.textContent = 'Copied!';
            setTimeout(() => {
                button.textContent = 'Copy';
            }, 2000);
        });
    });
    
    pre.style.position = 'relative';
    pre.appendChild(button);
});

// Add styles for buttons
const style = document.createElement('style');
style.textContent = `
    .try-button, .copy-button {
        position: absolute;
        top: 5px;
        padding: 5px 10px;
        font-size: 0.875em;
        border: 1px solid var(--border-color);
        border-radius: 4px;
        background: var(--bg);
        color: var(--fg);
        cursor: pointer;
        transition: all 0.2s;
    }
    
    .try-button {
        right: 60px;
    }
    
    .copy-button {
        right: 5px;
    }
    
    .try-button:hover, .copy-button:hover {
        background: var(--theme-hover);
    }
    
    .osv-line {
        position: relative;
    }
    
    .osv-line::after {
        content: "OSV";
        position: absolute;
        right: -50px;
        font-size: 0.75em;
        color: var(--rl-primary);
        opacity: 0.3;
    }
`;
document.head.appendChild(style);