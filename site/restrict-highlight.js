(function(global) {
    const KEYWORDS = new Set([
        'fun', 'val', 'mut', 'record', 'context', 'enum', 'match', 'then',
        'else', 'while', 'temporal', 'within', 'where', 'clone', 'freeze',
        'pub', 'import', 'export', 'impl', 'as', 'fatal', 'with',
        'lifetime', 'await', 'spawn'
    ]);
    const LITERALS = new Set(['true', 'false', 'Some', 'None', 'Ok', 'Err']);
    const TYPES = new Set([
        'Int32', 'Int64', 'Float64', 'String', 'Char', 'Boolean', 'List',
        'Array', 'Option', 'Result', 'Range'
    ]);
    const OPERATORS = [
        '|>', '=>', '->', '==', '!=', '<=', '>=', '&&', '||',
        '+', '-', '*', '/', '%', '=', '<', '>', '!', '~', ':', '.', ','
    ];

    function escapeHtml(value) {
        return value
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    function span(className, value) {
        return `<span class="${className}">${escapeHtml(value)}</span>`;
    }

    function readWhile(source, index, predicate) {
        let cursor = index;
        while (cursor < source.length && predicate(source[cursor])) {
            cursor += 1;
        }
        return cursor;
    }

    function isIdentifierStart(char) {
        return /[A-Za-z_]/.test(char);
    }

    function isIdentifierContinue(char) {
        return /[A-Za-z0-9_]/.test(char);
    }

    function isNumberChar(char) {
        return /[A-Za-z0-9_.]/.test(char);
    }

    function readQuoted(source, index, quote) {
        let cursor = index + 1;
        let escaped = false;
        while (cursor < source.length) {
            const char = source[cursor];
            cursor += 1;
            if (escaped) {
                escaped = false;
                continue;
            }
            if (char === '\\') {
                escaped = true;
                continue;
            }
            if (char === quote) {
                break;
            }
        }
        return cursor;
    }

    function readBlockComment(source, index) {
        const end = source.indexOf('*/', index + 2);
        return end === -1 ? source.length : end + 2;
    }

    function readLineComment(source, index) {
        const end = source.indexOf('\n', index + 2);
        return end === -1 ? source.length : end;
    }

    function matchingOperator(source, index) {
        return OPERATORS.find(operator => source.startsWith(operator, index));
    }

    function highlightRestrict(source) {
        let output = '';
        let index = 0;
        let expectTitle = false;

        while (index < source.length) {
            if (source.startsWith('//', index)) {
                const end = readLineComment(source, index);
                output += span('hljs-comment', source.slice(index, end));
                index = end;
                continue;
            }

            if (source.startsWith('/*', index)) {
                const end = readBlockComment(source, index);
                output += span('hljs-comment', source.slice(index, end));
                index = end;
                continue;
            }

            const char = source[index];

            if (char === '"' || char === "'") {
                const end = readQuoted(source, index, char);
                output += span('hljs-string', source.slice(index, end));
                index = end;
                continue;
            }

            if (/[0-9]/.test(char)) {
                const end = readWhile(source, index, isNumberChar);
                output += span('hljs-number', source.slice(index, end));
                index = end;
                continue;
            }

            if (isIdentifierStart(char)) {
                const end = readWhile(source, index, isIdentifierContinue);
                const word = source.slice(index, end);
                if (expectTitle) {
                    output += span('hljs-title', word);
                    expectTitle = false;
                } else if (KEYWORDS.has(word)) {
                    output += span('hljs-keyword', word);
                    expectTitle = word === 'fun' || word === 'record' || word === 'context' || word === 'impl';
                } else if (LITERALS.has(word)) {
                    output += span('hljs-literal', word);
                } else if (TYPES.has(word)) {
                    output += span('hljs-type', word);
                } else {
                    output += escapeHtml(word);
                }
                index = end;
                continue;
            }

            const operator = matchingOperator(source, index);
            if (operator) {
                output += span('hljs-operator', operator);
                index += operator.length;
                continue;
            }

            output += escapeHtml(char);
            index += 1;
        }

        return output;
    }

    function registerHighlightJs(hljs) {
        if (!hljs || hljs.getLanguage('restrict')) {
            return;
        }

        hljs.registerLanguage('restrict', function(hljsApi) {
            const typeMode = {
                className: 'type',
                begin: /\b(?:Int32|Int64|Float64|String|Char|Boolean|List|Array|Option|Result|Range)\b/
            };
            const operatorMode = {
                className: 'operator',
                begin: /\|>|=>|->|==|!=|<=|>=|&&|\|\||[+\-*\/%=<>!~:.,]/
            };

            return {
                name: 'Restrict',
                aliases: ['rl', 'restrict_lang'],
                keywords: {
                    keyword: Array.from(KEYWORDS).join(' '),
                    literal: Array.from(LITERALS).join(' ')
                },
                contains: [
                    hljsApi.C_LINE_COMMENT_MODE,
                    hljsApi.C_BLOCK_COMMENT_MODE,
                    hljsApi.QUOTE_STRING_MODE,
                    {
                        className: 'string',
                        begin: /'/,
                        end: /'/,
                        contains: [hljsApi.BACKSLASH_ESCAPE]
                    },
                    {
                        className: 'function',
                        beginKeywords: 'fun',
                        end: /=/,
                        excludeEnd: true,
                        contains: [
                            { className: 'title', begin: /[A-Za-z_][A-Za-z0-9_]*/ },
                            typeMode,
                            operatorMode
                        ]
                    },
                    {
                        className: 'class',
                        beginKeywords: 'record context impl',
                        end: /\{/,
                        excludeEnd: true,
                        contains: [
                            { className: 'title', begin: /[A-Za-z_][A-Za-z0-9_]*/ },
                            typeMode
                        ]
                    },
                    typeMode,
                    hljsApi.C_NUMBER_MODE,
                    operatorMode
                ]
            };
        });
    }

    global.RestrictHighlight = {
        escapeHtml,
        highlightRestrict,
        registerHighlightJs
    };

    registerHighlightJs(global.hljs);
})(window);
