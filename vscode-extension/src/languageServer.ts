import * as vscode from 'vscode';
import * as path from 'path';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind, ErrorAction, CloseAction, ErrorHandlerResult, CloseHandlerResult, Message } from 'vscode-languageclient/node';

let client: LanguageClient;

export function activateLanguageServer(context: vscode.ExtensionContext) {
    // The server is implemented in Rust and should be built as a separate binary
    const config = vscode.workspace.getConfiguration('restrict');
    let serverPath = config.get('languageServerPath') as string;
    
    if (!serverPath) {
        // Try to find the language server in common locations
        const possiblePaths = [
            'restrict_lsp',
            'restrict-lsp',
            './target/release/restrict_lsp',
            '../target/release/restrict_lsp'
        ];
        
        // For now, we'll use the compiler with LSP mode
        serverPath = config.get('compilerPath', 'restrict_lang');
    }

    // Server options - how to start the language server
    const serverOptions: ServerOptions = {
        command: serverPath,
        args: ['--lsp'],
        options: {
            env: {
                ...process.env,
                RUST_LOG: 'debug' // Enable debug logging for development
            }
        }
    };

    // Client options - language client configuration
    const clientOptions: LanguageClientOptions = {
        // Register the server for Restrict Language documents
        documentSelector: [
            { scheme: 'file', language: 'restrict' },
            { scheme: 'untitled', language: 'restrict' }
        ],
        
        synchronize: {
            // Notify the server about file changes to .rl files in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.rl')
        },

        // Output channel for LSP communication debugging
        outputChannel: vscode.window.createOutputChannel('Restrict Language Server'),

        // Error handling
        errorHandler: {
            error: (error: Error, message: Message | undefined, count: number | undefined): ErrorHandlerResult => {
                console.error('Language server error:', error, message, count);
                // Return ErrorAction based on the severity
                if ((count || 0) < 3) {
                    return { action: ErrorAction.Continue };
                }
                return { action: ErrorAction.Shutdown };
            },
            closed: (): CloseHandlerResult => {
                console.log('Language server connection closed');
                return { action: CloseAction.DoNotRestart };
            }
        }
    };

    // Create the language client
    client = new LanguageClient(
        'restrictLanguageServer',
        'Restrict Language Server',
        serverOptions,
        clientOptions
    );

    // Register custom LSP commands
    registerLSPCommands(context);

    // Start the client (and server)
    client.start().catch(error => {
        console.error('Failed to start language server:', error);
        vscode.window.showErrorMessage(`Failed to start Restrict Language Server: ${error.message}`);
    });

    // Register for disposal
    context.subscriptions.push(client);
}

export function deactivateLanguageServer(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

function registerLSPCommands(context: vscode.ExtensionContext) {
    // Go to definition
    const gotoDefinition = vscode.commands.registerCommand('restrict.gotoDefinition', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        const position = editor.selection.active;
        const locations = await vscode.commands.executeCommand<vscode.Location[]>(
            'vscode.executeDefinitionProvider',
            editor.document.uri,
            position
        );

        if (locations && locations.length > 0) {
            await vscode.window.showTextDocument(locations[0].uri, {
                selection: locations[0].range
            });
        } else {
            vscode.window.showInformationMessage('No definition found');
        }
    });

    // Find all references
    const findReferences = vscode.commands.registerCommand('restrict.findReferences', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        const position = editor.selection.active;
        await vscode.commands.executeCommand(
            'vscode.executeReferenceProvider',
            editor.document.uri,
            position
        );
    });

    // Rename symbol
    const renameSymbol = vscode.commands.registerCommand('restrict.renameSymbol', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        await vscode.commands.executeCommand('editor.action.rename');
    });

    // Document formatting
    const formatDocument = vscode.commands.registerCommand('restrict.formatDocument', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        await vscode.commands.executeCommand('editor.action.formatDocument');
    });

    // Hover information
    const showHover = vscode.commands.registerCommand('restrict.showHover', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        const position = editor.selection.active;
        const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
            'vscode.executeHoverProvider',
            editor.document.uri,
            position
        );

        if (hovers && hovers.length > 0) {
            const hover = hovers[0];
            if (hover.contents.length > 0) {
                const content = hover.contents[0];
                if (typeof content === 'string') {
                    vscode.window.showInformationMessage(content);
                } else if (content instanceof vscode.MarkdownString) {
                    vscode.window.showInformationMessage(content.value);
                }
            }
        }
    });

    // Document symbols (outline)
    const showDocumentSymbols = vscode.commands.registerCommand('restrict.showDocumentSymbols', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        await vscode.commands.executeCommand('workbench.action.gotoSymbol');
    });

    // Workspace symbols
    const showWorkspaceSymbols = vscode.commands.registerCommand('restrict.showWorkspaceSymbols', async () => {
        await vscode.commands.executeCommand('workbench.action.showAllSymbols');
    });

    // Code actions (quick fixes)
    const showCodeActions = vscode.commands.registerCommand('restrict.showCodeActions', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        await vscode.commands.executeCommand('editor.action.quickFix');
    });

    // Signature help
    const showSignatureHelp = vscode.commands.registerCommand('restrict.showSignatureHelp', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'restrict') {
            return;
        }

        await vscode.commands.executeCommand('editor.action.triggerParameterHints');
    });

    context.subscriptions.push(
        gotoDefinition,
        findReferences,
        renameSymbol,
        formatDocument,
        showHover,
        showDocumentSymbols,
        showWorkspaceSymbols,
        showCodeActions,
        showSignatureHelp
    );
}

// Custom LSP request handlers
export function registerCustomRequests(client: LanguageClient) {
    // Custom request for getting semantic tokens
    client.onRequest('restrict/semanticTokens', async (params: any) => {
        // Handle semantic tokens request
        return null;
    });

    // Custom request for getting inlay hints (type annotations)
    client.onRequest('restrict/inlayHints', async (params: any) => {
        // Handle inlay hints request
        return [];
    });

    // Custom request for getting type information
    client.onRequest('restrict/typeInfo', async (params: any) => {
        // Handle type information request
        return null;
    });
}