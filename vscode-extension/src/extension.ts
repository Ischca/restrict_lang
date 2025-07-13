import * as vscode from 'vscode';
import { exec } from 'child_process';
import { promisify } from 'util';
import * as path from 'path';
import * as fs from 'fs';
import { activateLanguageServer, deactivateLanguageServer } from './languageServer';
import { RestrictSemanticTokensProvider, legend } from './semanticTokens';

const execAsync = promisify(exec);

// Global reference to diagnostics collection
let diagnosticsCollection: vscode.DiagnosticCollection;

export function activate(context: vscode.ExtensionContext) {
    console.log('Restrict Language extension is now active!');

    // Initialize diagnostics collection
    diagnosticsCollection = vscode.languages.createDiagnosticCollection('restrict');
    context.subscriptions.push(diagnosticsCollection);

    // Commands will be handled by the Language Server when available
    // Register command proxies that will delegate to LSP
    const compileCommand = vscode.commands.registerCommand('restrict.compile', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            vscode.window.showErrorMessage('No active Restrict Language file');
            return;
        }
        
        // Check if LSP is enabled
        const config = vscode.workspace.getConfiguration('restrict');
        const enableLSP = config.get('enableLSP', false);
        
        if (enableLSP) {
            // Use LSP command
            const result = await vscode.commands.executeCommand('restrict.lsp.compile', editor.document.uri.toString()) as any;
            if (result) {
                if (result.success) {
                    vscode.window.showInformationMessage(result.message || 'Compilation completed');
                } else {
                    vscode.window.showErrorMessage(result.message || 'Compilation failed');
                }
            }
        } else {
            // Fallback to direct compilation
            compileCurrentFile();
        }
    });

    const typeCheckCommand = vscode.commands.registerCommand('restrict.typeCheck', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            vscode.window.showErrorMessage('No active Restrict Language file');
            return;
        }
        
        const config = vscode.workspace.getConfiguration('restrict');
        const enableLSP = config.get('enableLSP', false);
        
        if (enableLSP) {
            // Use LSP command
            const result = await vscode.commands.executeCommand('restrict.lsp.typeCheck', editor.document.uri.toString()) as any;
            if (result) {
                if (result.success) {
                    vscode.window.showInformationMessage(result.message || 'Type check passed');
                } else {
                    vscode.window.showErrorMessage(result.message || 'Type check failed');
                }
            }
        } else {
            // Fallback to direct type checking
            typeCheckCurrentFile();
        }
    });

    const showASTCommand = vscode.commands.registerCommand('restrict.showAST', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            vscode.window.showErrorMessage('No active Restrict Language file');
            return;
        }
        
        const config = vscode.workspace.getConfiguration('restrict');
        const enableLSP = config.get('enableLSP', false);
        
        if (enableLSP) {
            // Use LSP command
            const result = await vscode.commands.executeCommand('restrict.lsp.showAST', editor.document.uri.toString()) as any;
            if (result && result.success) {
                // Show AST in a new document
                const doc = await vscode.workspace.openTextDocument({
                    content: result.ast,
                    language: 'json'
                });
                await vscode.window.showTextDocument(doc);
            } else {
                vscode.window.showErrorMessage(result?.message || 'Failed to generate AST');
            }
        } else {
            // Fallback to direct AST generation
            showAST();
        }
    });

    // Register document save handler for auto type checking
    const onSaveHandler = vscode.workspace.onDidSaveTextDocument((document) => {
        if (document.languageId === 'restrict') {
            const config = vscode.workspace.getConfiguration('restrict');
            if (config.get('enableTypeChecking')) {
                typeCheckFile(document.fileName, false);
            }
        }
    });

    // Register semantic tokens provider
    const semanticTokensProvider = new RestrictSemanticTokensProvider();
    const semanticTokensRegistration = vscode.languages.registerDocumentSemanticTokensProvider(
        { language: 'restrict' },
        semanticTokensProvider,
        legend
    );

    context.subscriptions.push(
        compileCommand,
        typeCheckCommand,
        showASTCommand,
        onSaveHandler,
        semanticTokensRegistration
    );

    // Activate Language Server Protocol
    const config = vscode.workspace.getConfiguration('restrict');
    const enableLSP = config.get('enableLSP', true);
    
    if (enableLSP) {
        try {
            activateLanguageServer(context);
            console.log('Language Server Protocol activated');
        } catch (error) {
            console.warn('Failed to activate Language Server Protocol:', error);
            // Continue with basic functionality even if LSP fails
        }
    } else {
        console.log('Language Server Protocol is disabled');
    }
}

export function deactivate() {
    return deactivateLanguageServer();
}

async function compileCurrentFile() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    if (editor.document.languageId !== 'restrict') {
        vscode.window.showErrorMessage('Current file is not a Restrict Language file');
        return;
    }

    const filePath = editor.document.fileName;
    const config = vscode.workspace.getConfiguration('restrict');
    let compilerPath = config.get('compilerPath') as string;
    
    // If no compiler path is configured, use 'restrict_lang' from PATH
    if (!compilerPath) {
        compilerPath = 'restrict_lang';
    }

    try {
        vscode.window.showInformationMessage('Compiling Restrict file...');
        
        const outputPath = filePath.replace(/\.rl$/, '.wat');
        const { stdout, stderr } = await execAsync(`"${compilerPath}" "${filePath}" "${outputPath}"`);
        
        if (stderr) {
            vscode.window.showErrorMessage(`Compilation error: ${stderr}`);
        } else {
            vscode.window.showInformationMessage(`Compilation successful! Output: ${outputPath}`);
            if (stdout) {
                console.log('Compiler output:', stdout);
            }
        }
    } catch (error: any) {
        vscode.window.showErrorMessage(`Compilation failed: ${error.message}`);
    }
}

async function typeCheckCurrentFile() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    if (editor.document.languageId !== 'restrict') {
        vscode.window.showErrorMessage('Current file is not a Restrict Language file');
        return;
    }

    await typeCheckFile(editor.document.fileName, true);
}

async function typeCheckFile(filePath: string, showMessage: boolean = false) {
    const config = vscode.workspace.getConfiguration('restrict');
    let compilerPath = config.get('compilerPath') as string;
    
    // If no compiler path is configured, use 'restrict_lang' from PATH
    if (!compilerPath) {
        compilerPath = 'restrict_lang';
    }

    try {
        const { stdout, stderr } = await execAsync(`"${compilerPath}" --check "${filePath}"`);
        
        // Clear previous diagnostics
        diagnosticsCollection.delete(vscode.Uri.file(filePath));

        if (stderr) {
            // Parse error messages and create diagnostics
            const diagnostics = parseErrorMessages(stderr);
            if (diagnostics.length > 0) {
                diagnosticsCollection.set(vscode.Uri.file(filePath), diagnostics);
            }
            
            if (showMessage) {
                vscode.window.showErrorMessage(`Type check failed: ${stderr}`);
            }
        } else {
            if (showMessage) {
                vscode.window.showInformationMessage('Type check passed!');
            }
        }
    } catch (error: any) {
        if (showMessage) {
            vscode.window.showErrorMessage(`Type check failed: ${error.message}`);
        }
    }
}

async function showAST() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    if (editor.document.languageId !== 'restrict') {
        vscode.window.showErrorMessage('Current file is not a Restrict Language file');
        return;
    }

    const filePath = editor.document.fileName;
    const config = vscode.workspace.getConfiguration('restrict');
    let compilerPath = config.get('compilerPath') as string;
    
    // If no compiler path is configured, use 'restrict_lang' from PATH
    if (!compilerPath) {
        compilerPath = 'restrict_lang';
    }

    try {
        const { stdout, stderr } = await execAsync(`"${compilerPath}" --ast "${filePath}"`);
        
        if (stderr) {
            vscode.window.showErrorMessage(`AST generation failed: ${stderr}`);
        } else {
            // Create a new document to show the AST
            const doc = await vscode.workspace.openTextDocument({
                content: stdout,
                language: 'json'
            });
            await vscode.window.showTextDocument(doc);
        }
    } catch (error: any) {
        vscode.window.showErrorMessage(`AST generation failed: ${error.message}`);
    }
}

function parseErrorMessages(errorOutput: string): vscode.Diagnostic[] {
    const diagnostics: vscode.Diagnostic[] = [];
    const lines = errorOutput.split('\n');

    for (const line of lines) {
        // Try to parse error format: "file.rl:line:col: error message"
        const match = line.match(/(.+):(\d+):(\d+):\s*(.+)/);
        if (match) {
            const [, , lineStr, colStr, message] = match;
            const lineNum = parseInt(lineStr) - 1; // Convert to 0-based
            const colNum = parseInt(colStr) - 1;
            
            const range = new vscode.Range(
                new vscode.Position(lineNum, colNum),
                new vscode.Position(lineNum, colNum + 1)
            );
            
            const diagnostic = new vscode.Diagnostic(
                range,
                message,
                vscode.DiagnosticSeverity.Error
            );
            
            diagnostics.push(diagnostic);
        } else if (line.trim() && !line.startsWith('  ')) {
            // Generic error without position info
            const range = new vscode.Range(
                new vscode.Position(0, 0),
                new vscode.Position(0, 1)
            );
            
            const diagnostic = new vscode.Diagnostic(
                range,
                line.trim(),
                vscode.DiagnosticSeverity.Error
            );
            
            diagnostics.push(diagnostic);
        }
    }

    return diagnostics;
}

// getDiagnosticsCollection function is no longer needed since we use global reference