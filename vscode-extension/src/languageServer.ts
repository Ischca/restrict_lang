import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import {
    CloseAction,
    CloseHandlerResult,
    ErrorAction,
    ErrorHandlerResult,
    LanguageClient,
    LanguageClientOptions,
    Message,
    ServerOptions,
    State
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let clientReady = false;
let outputChannel: vscode.OutputChannel | undefined;
let fileWatcher: vscode.FileSystemWatcher | undefined;

function executableName(): string {
    return process.platform === 'win32' ? 'restrict_lang.exe' : 'restrict_lang';
}

function isPath(command: string): boolean {
    return path.isAbsolute(command) || command.includes('/') || command.includes('\\');
}

function developmentServerPath(context: vscode.ExtensionContext): string | undefined {
    const repositoryRoot = path.resolve(context.extensionPath, '..');
    if (!fs.existsSync(path.join(repositoryRoot, 'Cargo.toml'))) {
        return undefined;
    }

    const executable = executableName();
    const candidates = [
        path.join(repositoryRoot, 'target', 'debug', executable),
        path.join(repositoryRoot, 'target', 'release', executable)
    ];
    return candidates.find(candidate => fs.existsSync(candidate));
}

export function resolveRestrictCommand(
    context: vscode.ExtensionContext,
    preferLanguageServerPath: boolean
): string {
    const config = vscode.workspace.getConfiguration('restrict');
    const languageServerPath = config.get<string>('languageServerPath', '').trim();
    const compilerPath = config.get<string>('compilerPath', '').trim();

    if (preferLanguageServerPath && languageServerPath) {
        return languageServerPath;
    }
    if (compilerPath) {
        return compilerPath;
    }
    if (languageServerPath) {
        return languageServerPath;
    }

    return developmentServerPath(context) ?? 'restrict_lang';
}

export function isLanguageServerReady(): boolean {
    return clientReady;
}

export async function activateLanguageServer(context: vscode.ExtensionContext): Promise<boolean> {
    const config = vscode.workspace.getConfiguration('restrict');
    if (!config.get<boolean>('enableLSP', true)) {
        return false;
    }
    if (clientReady) {
        return true;
    }

    const serverCommand = resolveRestrictCommand(context, true);
    if (isPath(serverCommand) && !fs.existsSync(serverCommand)) {
        vscode.window.showWarningMessage(
            `Restrict language server was not found at ${serverCommand}. ` +
            'Set restrict.languageServerPath or build the compiler.'
        );
        return false;
    }

    if (!outputChannel) {
        outputChannel = vscode.window.createOutputChannel('Restrict Language Server');
        context.subscriptions.push(outputChannel);
    }
    fileWatcher = vscode.workspace.createFileSystemWatcher('**/*.rl');

    const workspaceDirectory = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    const serverOptions: ServerOptions = {
        command: serverCommand,
        args: ['--lsp'],
        options: {
            cwd: workspaceDirectory,
            env: process.env
        }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'restrict' },
            { scheme: 'untitled', language: 'restrict' }
        ],
        synchronize: {
            fileEvents: fileWatcher
        },
        outputChannel,
        errorHandler: {
            error: (
                error: Error,
                message: Message | undefined,
                count: number | undefined
            ): ErrorHandlerResult => {
                outputChannel?.appendLine(
                    `Language server error (${count ?? 1}): ${error.message}` +
                    (message ? `; message: ${JSON.stringify(message)}` : '')
                );
                return { action: (count ?? 0) < 3 ? ErrorAction.Continue : ErrorAction.Shutdown };
            },
            closed: (): CloseHandlerResult => {
                clientReady = false;
                return { action: CloseAction.Restart };
            }
        }
    };

    client = new LanguageClient(
        'restrictLanguageServer',
        'Restrict Language Server',
        serverOptions,
        clientOptions
    );
    const stateSubscription = client.onDidChangeState(event => {
        clientReady = event.newState === State.Running;
    });

    try {
        await client.start();
        clientReady = true;
        context.subscriptions.push(client, fileWatcher, stateSubscription);
        outputChannel.appendLine(`Started ${serverCommand} --lsp`);
        return true;
    } catch (error) {
        clientReady = false;
        const message = error instanceof Error ? error.message : String(error);
        outputChannel.appendLine(`Failed to start language server: ${message}`);
        vscode.window.showWarningMessage(
            `Restrict language server could not start: ${message}. Compiler commands will use the CLI fallback.`
        );
        client = undefined;
        stateSubscription.dispose();
        fileWatcher.dispose();
        fileWatcher = undefined;
        return false;
    }
}

export async function restartLanguageServer(context: vscode.ExtensionContext): Promise<boolean> {
    await deactivateLanguageServer();
    return activateLanguageServer(context);
}

export async function deactivateLanguageServer(): Promise<void> {
    clientReady = false;
    const activeClient = client;
    client = undefined;
    if (activeClient) {
        await activeClient.stop();
    }
    fileWatcher?.dispose();
    fileWatcher = undefined;
}

export async function executeLanguageServerCommand<T>(
    command: string,
    args: unknown[]
): Promise<T | undefined> {
    if (!client || !clientReady) {
        return undefined;
    }

    return client.sendRequest<T>('workspace/executeCommand', {
        command,
        arguments: args
    });
}
