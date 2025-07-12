import * as vscode from 'vscode';

// Define semantic token types for Restrict Language
export const tokenTypes = [
    'namespace',
    'class',
    'enum',
    'interface',
    'struct',
    'typeParameter',
    'type',
    'parameter',
    'variable',
    'property',
    'enumMember',
    'decorator',
    'event',
    'function',
    'method',
    'macro',
    'label',
    'comment',
    'string',
    'keyword',
    'number',
    'regexp',
    'operator',
    // Restrict Language specific tokens
    'record',
    'lambda',
    'pattern',
    'pipe',
    'arena'
];

export const tokenModifiers = [
    'declaration',
    'definition',
    'readonly',
    'static',
    'deprecated',
    'abstract',
    'async',
    'modification',
    'documentation',
    'defaultLibrary',
    // Restrict Language specific modifiers
    'mutable',
    'frozen',
    'captured',
    'affine'
];

export const legend = new vscode.SemanticTokensLegend(tokenTypes, tokenModifiers);

export class RestrictSemanticTokensProvider implements vscode.DocumentSemanticTokensProvider {
    async provideDocumentSemanticTokens(
        document: vscode.TextDocument,
        token: vscode.CancellationToken
    ): Promise<vscode.SemanticTokens> {
        const tokensBuilder = new vscode.SemanticTokensBuilder(legend);
        
        // Parse the document and generate semantic tokens
        const text = document.getText();
        const lines = text.split('\n');
        
        for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
            const line = lines[lineIndex];
            await this.parseLineForTokens(line, lineIndex, tokensBuilder);
            
            if (token.isCancellationRequested) {
                return tokensBuilder.build();
            }
        }
        
        return tokensBuilder.build();
    }
    
    private async parseLineForTokens(
        line: string,
        lineIndex: number,
        tokensBuilder: vscode.SemanticTokensBuilder
    ): Promise<void> {
        // Simple regex-based tokenization
        // In a real implementation, this would use the AST from the language server
        
        // Function definitions
        const funMatch = /\b(fun)\s+(\w+)/g;
        let match;
        while ((match = funMatch.exec(line)) !== null) {
            // 'fun' keyword
            tokensBuilder.push(
                lineIndex,
                match.index,
                match[1].length,
                this.getTokenType('keyword'),
                this.getTokenModifiers(['declaration'])
            );
            
            // Function name
            tokensBuilder.push(
                lineIndex,
                match.index + match[1].length + 1,
                match[2].length,
                this.getTokenType('function'),
                this.getTokenModifiers(['declaration'])
            );
        }
        
        // Record definitions
        const recordMatch = /\b(record)\s+(\w+)/g;
        while ((match = recordMatch.exec(line)) !== null) {
            // 'record' keyword
            tokensBuilder.push(
                lineIndex,
                match.index,
                match[1].length,
                this.getTokenType('keyword'),
                this.getTokenModifiers(['declaration'])
            );
            
            // Record name
            tokensBuilder.push(
                lineIndex,
                match.index + match[1].length + 1,
                match[2].length,
                this.getTokenType('record'),
                this.getTokenModifiers(['declaration'])
            );
        }
        
        // Lambda expressions
        const lambdaMatch = /\|([^|]*)\|/g;
        while ((match = lambdaMatch.exec(line)) !== null) {
            // Lambda pipes
            tokensBuilder.push(
                lineIndex,
                match.index,
                1,
                this.getTokenType('operator'),
                this.getTokenModifiers([])
            );
            
            tokensBuilder.push(
                lineIndex,
                match.index + match[0].length - 1,
                1,
                this.getTokenType('operator'),
                this.getTokenModifiers([])
            );
            
            // Lambda parameters
            const params = match[1].split(',').map(p => p.trim()).filter(p => p.length > 0);
            let paramOffset = match.index + 1;
            for (const param of params) {
                const paramIndex = line.indexOf(param, paramOffset);
                if (paramIndex !== -1) {
                    tokensBuilder.push(
                        lineIndex,
                        paramIndex,
                        param.length,
                        this.getTokenType('parameter'),
                        this.getTokenModifiers(['lambda'])
                    );
                    paramOffset = paramIndex + param.length;
                }
            }
        }
        
        // Variable bindings
        const valMatch = /\b(val)\s+(mut\s+)?(\w+)/g;
        while ((match = valMatch.exec(line)) !== null) {
            // 'val' keyword
            tokensBuilder.push(
                lineIndex,
                match.index,
                match[1].length,
                this.getTokenType('keyword'),
                this.getTokenModifiers(['declaration'])
            );
            
            // 'mut' keyword if present
            if (match[2]) {
                const mutIndex = match.index + match[1].length + 1;
                tokensBuilder.push(
                    lineIndex,
                    mutIndex,
                    3, // 'mut'.length
                    this.getTokenType('keyword'),
                    this.getTokenModifiers(['modification'])
                );
            }
            
            // Variable name
            const varNameIndex = match.index + match[1].length + (match[2] ? match[2].length : 0) + 1;
            tokensBuilder.push(
                lineIndex,
                varNameIndex,
                match[3].length,
                this.getTokenType('variable'),
                this.getTokenModifiers(match[2] ? ['declaration', 'mutable'] : ['declaration'])
            );
        }
        
        // Type annotations
        const typeMatch = /:\s*(\w+)(?:<([^>]+)>)?/g;
        while ((match = typeMatch.exec(line)) !== null) {
            // Main type
            const typeIndex = line.indexOf(match[1], match.index);
            tokensBuilder.push(
                lineIndex,
                typeIndex,
                match[1].length,
                this.getTokenType('type'),
                this.getTokenModifiers([])
            );
            
            // Generic type parameters
            if (match[2]) {
                const genericIndex = line.indexOf(match[2], typeIndex + match[1].length);
                tokensBuilder.push(
                    lineIndex,
                    genericIndex,
                    match[2].length,
                    this.getTokenType('typeParameter'),
                    this.getTokenModifiers([])
                );
            }
        }
        
        // Pipe operators
        const pipeMatch = /(\|>>?)/g;
        while ((match = pipeMatch.exec(line)) !== null) {
            tokensBuilder.push(
                lineIndex,
                match.index,
                match[1].length,
                this.getTokenType('pipe'),
                this.getTokenModifiers([])
            );
        }
        
        // Pattern matching wildcards
        const wildcardMatch = /\b_\b/g;
        while ((match = wildcardMatch.exec(line)) !== null) {
            tokensBuilder.push(
                lineIndex,
                match.index,
                1,
                this.getTokenType('pattern'),
                this.getTokenModifiers([])
            );
        }
    }
    
    private getTokenType(type: string): number {
        const index = tokenTypes.indexOf(type);
        return index === -1 ? 0 : index;
    }
    
    private getTokenModifiers(modifiers: string[]): number {
        let result = 0;
        for (const modifier of modifiers) {
            const index = tokenModifiers.indexOf(modifier);
            if (index !== -1) {
                result |= (1 << index);
            }
        }
        return result;
    }
}