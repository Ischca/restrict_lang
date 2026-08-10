import { execFile } from 'child_process';
import * as path from 'path';
import * as vscode from 'vscode';
import {
    activateLanguageServer,
    deactivateLanguageServer,
    executeLanguageServerCommand,
    isLanguageServerReady,
    resolveRestrictCommand,
    restartLanguageServer
} from './languageServer';

interface RestrictCommandResult {
    success: boolean;
    message?: string;
    ast?: string;
}

interface ProcessResult {
    stdout: string;
    stderr: string;
}

class CompilerError extends Error {
    constructor(message: string, readonly stderr: string) {
        super(message);
    }
}

let diagnosticsCollection: vscode.DiagnosticCollection;
let extensionContext: vscode.ExtensionContext;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    extensionContext = context;
    diagnosticsCollection = vscode.languages.createDiagnosticCollection('restrict-cli');

    context.subscriptions.push(
        diagnosticsCollection,
        vscode.commands.registerCommand('restrict.compile', compileCurrentFile),
        vscode.commands.registerCommand('restrict.typeCheck', () => typeCheckCurrentFile(true)),
        vscode.commands.registerCommand('restrict.showAST', showAST),
        vscode.commands.registerCommand('restrict.gotoDefinition', () =>
            vscode.commands.executeCommand('editor.action.revealDefinition')
        ),
        vscode.commands.registerCommand('restrict.findReferences', () =>
            vscode.commands.executeCommand('editor.action.goToReferences')
        ),
        vscode.commands.registerCommand('restrict.showHover', () =>
            vscode.commands.executeCommand('editor.action.showHover')
        ),
        vscode.commands.registerCommand('restrict.showDocumentSymbols', () =>
            vscode.commands.executeCommand('workbench.action.gotoSymbol')
        ),
        vscode.commands.registerCommand('restrict.restartLanguageServer', async () => {
            const started = await restartLanguageServer(context);
            if (started) {
                vscode.window.showInformationMessage('Restrict language server restarted.');
            }
        }),
        vscode.workspace.onDidSaveTextDocument(async document => {
            const config = vscode.workspace.getConfiguration('restrict');
            if (
                document.languageId === 'restrict' &&
                config.get<boolean>('enableTypeChecking', true) &&
                !isLanguageServerReady()
            ) {
                await typeCheckFile(document, false);
            }
        })
    );

    await activateLanguageServer(context);
}

export async function deactivate(): Promise<void> {
    await deactivateLanguageServer();
}

function activeRestrictDocument(): vscode.TextDocument | undefined {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== 'restrict') {
        vscode.window.showErrorMessage('No active Restrict Language file.');
        return undefined;
    }
    return editor.document;
}

async function compileCurrentFile(): Promise<void> {
    const document = activeRestrictDocument();
    if (!document) {
        return;
    }

    const lspResult = await executeLanguageServerCommand<RestrictCommandResult>(
        'restrict.lsp.compile',
        [document.uri.toString()]
    );
    if (lspResult !== undefined) {
        showCommandResult(lspResult, 'Compilation completed.', 'Compilation failed.');
        return;
    }

    if (!(await ensureSaved(document))) {
        return;
    }

    const outputPath = document.fileName.replace(/\.rl$/, '.wat');
    try {
        await runCompiler([document.fileName, outputPath]);
        vscode.window.showInformationMessage(`Compilation succeeded: ${outputPath}`);
    } catch (error) {
        showCompilerError('Compilation failed', error);
    }
}

async function typeCheckCurrentFile(showMessage: boolean): Promise<void> {
    const document = activeRestrictDocument();
    if (document) {
        await typeCheckFile(document, showMessage);
    }
}

async function typeCheckFile(document: vscode.TextDocument, showMessage: boolean): Promise<void> {
    const lspResult = await executeLanguageServerCommand<RestrictCommandResult>(
        'restrict.lsp.typeCheck',
        [document.uri.toString()]
    );
    if (lspResult !== undefined) {
        if (showMessage) {
            showCommandResult(lspResult, 'Type check passed.', 'Type check failed.');
        }
        return;
    }

    if (!(await ensureSaved(document))) {
        return;
    }

    diagnosticsCollection.delete(document.uri);
    try {
        await runCompiler(['--check', document.fileName]);
        if (showMessage) {
            vscode.window.showInformationMessage('Type check passed.');
        }
    } catch (error) {
        const stderr = error instanceof CompilerError ? error.stderr : String(error);
        const diagnostics = parseErrorMessages(stderr);
        if (diagnostics.length > 0) {
            diagnosticsCollection.set(document.uri, diagnostics);
        }
        if (showMessage) {
            showCompilerError('Type check failed', error);
        }
    }
}

async function showAST(): Promise<void> {
    const document = activeRestrictDocument();
    if (!document) {
        return;
    }

    const lspResult = await executeLanguageServerCommand<RestrictCommandResult>(
        'restrict.lsp.showAST',
        [document.uri.toString()]
    );
    if (lspResult !== undefined) {
        if (!lspResult.success || !lspResult.ast) {
            showCommandResult(lspResult, '', 'AST generation failed.');
            return;
        }
        await showTextDocument(lspResult.ast);
        return;
    }

    if (!(await ensureSaved(document))) {
        return;
    }

    try {
        const result = await runCompiler(['--ast', document.fileName]);
        await showTextDocument(result.stdout);
    } catch (error) {
        showCompilerError('AST generation failed', error);
    }
}

async function showTextDocument(content: string): Promise<void> {
    const document = await vscode.workspace.openTextDocument({ content, language: 'plaintext' });
    await vscode.window.showTextDocument(document);
}

async function ensureSaved(document: vscode.TextDocument): Promise<boolean> {
    if (document.uri.scheme !== 'file') {
        vscode.window.showErrorMessage('Save the Restrict file before using the CLI fallback.');
        return false;
    }
    if (document.isDirty && !(await document.save())) {
        vscode.window.showErrorMessage('The Restrict file could not be saved.');
        return false;
    }
    return true;
}

function runCompiler(args: string[]): Promise<ProcessResult> {
    const command = resolveRestrictCommand(extensionContext, false);
    const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? path.dirname(args.at(-1) ?? '.');

    return new Promise((resolve, reject) => {
        execFile(command, args, { cwd, maxBuffer: 10 * 1024 * 1024 }, (error, stdout, stderr) => {
            const result = { stdout: stdout.toString(), stderr: stderr.toString() };
            if (error) {
                reject(new CompilerError(error.message, result.stderr || result.stdout));
                return;
            }
            resolve(result);
        });
    });
}

function showCommandResult(
    result: RestrictCommandResult,
    successFallback: string,
    failureFallback: string
): void {
    const message = result.message || (result.success ? successFallback : failureFallback);
    if (result.success) {
        vscode.window.showInformationMessage(message);
    } else {
        vscode.window.showErrorMessage(message);
    }
}

function showCompilerError(prefix: string, error: unknown): void {
    const detail = error instanceof CompilerError
        ? error.stderr || error.message
        : error instanceof Error
            ? error.message
            : String(error);
    vscode.window.showErrorMessage(`${prefix}: ${detail.trim()}`);
}

function parseErrorMessages(errorOutput: string): vscode.Diagnostic[] {
    const diagnostics: vscode.Diagnostic[] = [];
    for (const line of errorOutput.split('\n')) {
        const match = line.match(/(?:.+:)?(\d+):(\d+):\s*(.+)/);
        if (match) {
            const lineNumber = Math.max(0, Number.parseInt(match[1], 10) - 1);
            const columnNumber = Math.max(0, Number.parseInt(match[2], 10) - 1);
            diagnostics.push(new vscode.Diagnostic(
                new vscode.Range(lineNumber, columnNumber, lineNumber, columnNumber + 1),
                match[3],
                vscode.DiagnosticSeverity.Error
            ));
            continue;
        }

        const descriptiveMatch = line.match(/line\s+(\d+),?\s+column\s+(\d+)/i);
        if (descriptiveMatch) {
            const lineNumber = Math.max(0, Number.parseInt(descriptiveMatch[1], 10) - 1);
            const columnNumber = Math.max(0, Number.parseInt(descriptiveMatch[2], 10) - 1);
            diagnostics.push(new vscode.Diagnostic(
                new vscode.Range(lineNumber, columnNumber, lineNumber, columnNumber + 1),
                line.trim(),
                vscode.DiagnosticSeverity.Error
            ));
        }
    }

    if (diagnostics.length === 0 && errorOutput.trim()) {
        diagnostics.push(new vscode.Diagnostic(
            new vscode.Range(0, 0, 0, 1),
            errorOutput.trim(),
            vscode.DiagnosticSeverity.Error
        ));
    }
    return diagnostics;
}
