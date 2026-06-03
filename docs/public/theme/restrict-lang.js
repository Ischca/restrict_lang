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
    });
}

function initializeInteractiveExamples() {
    // Add "Try it" buttons to code examples
    document.querySelectorAll('pre code.language-restrict').forEach((block, index) => {
        const pre = block.parentElement;
        const code = block.textContent;

        // Skip if it's not a complete example
        if (!code.includes('fun main')) {
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
    const playgroundUrl = `${pagesSiteRoot()}compiler/`;
    const encoded = encodeURIComponent(code);
    window.open(`${playgroundUrl}?code=${encoded}`, '_blank');
}

function pagesSiteRoot() {
    const pathname = window.location.pathname;
    const docsIndex = pathname.indexOf('/docs/');

    if (docsIndex >= 0) {
        return pathname.slice(0, docsIndex + 1);
    }

    if (pathname.endsWith('/docs')) {
        return pathname.slice(0, -'docs'.length);
    }

    return '../';
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

`;
document.head.appendChild(style);
