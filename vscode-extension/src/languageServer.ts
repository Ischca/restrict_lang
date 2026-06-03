import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind, ErrorAction, CloseAction, ErrorHandlerResult, CloseHandlerResult, Message } from 'vscode-languageclient/node';

let client: LanguageClient;

function getServerCommand(config: vscode.WorkspaceConfiguration): string {
    const languageServerPath = config.get<string>('languageServerPath', '').trim();
    if (languageServerPath) {
        return languageServerPath;
    }

    const compilerPath = config.get<string>('compilerPath', '').trim();
    return compilerPath || 'restrict_lang';
}

function shouldCheckLocalPath(command: string): boolean {
    return path.isAbsolute(command) || command.includes('/') || command.includes('\\');
}

export function activateLanguageServer(context: vscode.ExtensionContext) {
    // Check if LSP is enabled in configuration
    const config = vscode.workspace.getConfiguration('restrict');
    const enableLSP = config.get('enableLSP', true);
    
    if (!enableLSP) {
        console.log('Language Server Protocol is disabled in configuration');
        return;
    }

    const serverCommand = getServerCommand(config);

    // Check configured filesystem paths, but allow bare commands to resolve from PATH.
    if (shouldCheckLocalPath(serverCommand) && !fs.existsSync(serverCommand)) {
        console.warn(`Language server binary not found at: ${serverCommand}`);
        vscode.window.showWarningMessage(`Restrict Language Server not found at ${serverCommand}. LSP features will be unavailable.`);
        return;
    }

    // Server options - how to start the language server
    const serverOptions: ServerOptions = {
        command: serverCommand,
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
        
        // Disable automatic command registration to avoid conflicts
        initializationOptions: {
            registerCommands: false
        },


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

    // Start the client (and server) first
    client.start().then(() => {
        // Register custom LSP commands after the client has started
        registerLSPCommands(context);
        console.log('Language Server started successfully');
    }).catch(error => {
        console.error('Failed to start language server:', error);
        
        // Check if it's a command registration conflict
        if (error.message && error.message.includes('already exists')) {
            console.warn('Command registration conflict detected - LSP may have conflicting commands');
            // Don't show error to user, just log it
            console.log('Continuing with existing command registrations');
        } else {
            vscode.window.showErrorMessage(`Failed to start Restrict Language Server: ${error.message}`);
        }
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
    // These are additional LSP-specific commands that don't conflict with the main commands
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

    context.subscriptions.push(
        gotoDefinition,
        findReferences,
        showHover,
        showDocumentSymbols
    );
}
