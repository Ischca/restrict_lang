import * as vscode from 'vscode';
import { exec } from 'child_process';
import { promisify } from 'util';
import * as path from 'path';
import * as fs from 'fs';
import { activateLanguageServer, deactivateLanguageServer } from './languageServer';
import { RestrictSemanticTokensProvider, legend } from './semanticTokens';

const execAsync = promisify(exec);

export function activate(context: vscode.ExtensionContext) {
    console.log('Restrict Language extension is now active!');

    // Initialize diagnostics collection
    const diagnosticsCollection = vscode.languages.createDiagnosticCollection('restrict');
    context.subscriptions.push(diagnosticsCollection);

    // Store diagnostics collection in context for use in other functions
    (context as any).diagnosticsCollection = diagnosticsCollection;

    // Activate Language Server Protocol
    try {
        activateLanguageServer(context);
        console.log('Language Server Protocol activated');
    } catch (error) {
        console.warn('Failed to activate Language Server Protocol:', error);
        // Continue with basic functionality even if LSP fails
    }

    // Register commands
    const compileCommand = vscode.commands.registerCommand('restrict.compile', () => {
        compileCurrentFile();
    });

    const typeCheckCommand = vscode.commands.registerCommand('restrict.typeCheck', () => {
        typeCheckCurrentFile();
    });

    const showASTCommand = vscode.commands.registerCommand('restrict.showAST', () => {
        showAST();
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
    const compilerPath = config.get('compilerPath', 'restrict_lang');

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
    const compilerPath = config.get('compilerPath', 'restrict_lang');

    try {
        const { stdout, stderr } = await execAsync(`"${compilerPath}" --check "${filePath}"`);
        
        // Clear previous diagnostics
        const diagnosticsCollection = getDiagnosticsCollection();
        if (diagnosticsCollection) {
            diagnosticsCollection.delete(vscode.Uri.file(filePath));
        }

        if (stderr) {
            // Parse error messages and create diagnostics
            const diagnostics = parseErrorMessages(stderr);
            if (diagnosticsCollection && diagnostics.length > 0) {
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
    const compilerPath = config.get('compilerPath', 'restrict_lang');

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

function getDiagnosticsCollection(): vscode.DiagnosticCollection | undefined {
    // This is a bit hacky, but we need to access the diagnostics collection
    // stored in the extension context. In a real implementation, you might
    // want to use a more robust pattern.
    const extension = vscode.extensions.getExtension('restrict-lang.restrict-language');
    if (extension && extension.isActive) {
        return (extension.exports as any)?.diagnosticsCollection;
    }
    return undefined;
}