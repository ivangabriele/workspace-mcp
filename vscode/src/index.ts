import { type ChildProcessWithoutNullStreams, spawn } from 'node:child_process'
import * as os from 'node:os'
import * as path from 'node:path'
import * as vscode from 'vscode'

let output: vscode.OutputChannel | undefined
let proc: ChildProcessWithoutNullStreams | undefined

function platformDir(): string {
  const plat = process.platform // 'darwin' | 'linux' | 'win32'
  const arch = process.arch // 'arm64' | 'x64' | ...
  if (plat === 'darwin' && arch === 'arm64') return 'darwin-arm64'
  if (plat === 'darwin') return 'darwin-x64'
  if (plat === 'win32') return 'win32-x64'
  return 'linux-x64'
}

function serverPath(ctx: vscode.ExtensionContext): string {
  const exe = process.platform === 'win32' ? 'mcp-rust.exe' : 'mcp-rust'
  return path.join(ctx.extensionPath, 'assets', platformDir(), exe)
}

// TODO Use cloudflared (`cloudflared tunnel run --token <token>`) to expose the MCP server.
async function start(ctx: vscode.ExtensionContext) {
  if (proc) {
    vscode.window.showInformationMessage('MCP server already running.')
    return
  }

  const bin = serverPath(ctx)
  const token = os.userInfo().username + ':' + Math.random().toString(36).slice(2, 10)

  output?.appendLine(`[mcp] launching: ${bin}`)

  proc = spawn(
    bin,
    [
      '--bind',
      '127.0.0.1:0', // choose any free port
      '--telemetry',
      '127.0.0.1:0', // separate telemetry socket
      '--workspace',
      vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '',
      '--auth-token',
      token,
    ],
    { stdio: 'pipe' },
  )

  // The server prints the chosen ports as JSON on stdout (see Rust code)
  proc.stdout.on('data', buf => {
    const line = buf.toString()
    output?.append(line)
  })

  proc.stderr.on('data', buf => {
    output?.append(`[stderr] ${buf.toString()}`)
  })

  proc.on('exit', (code, sig) => {
    output?.appendLine(`[mcp] exited code=${code} signal=${sig}`)
    proc = undefined
  })

  vscode.window.showInformationMessage('MCP server started. Use "MCP: Show logs" to view.')
}

async function stop() {
  if (!proc) {
    vscode.window.showInformationMessage('MCP server is not running.')
    return
  }
  proc.kill('SIGTERM')
}

export async function activate(ctx: vscode.ExtensionContext) {
  output = vscode.window.createOutputChannel('Workspace MCP')
  ctx.subscriptions.push(output)

  ctx.subscriptions.push(
    vscode.commands.registerCommand('mcp.start', () => start(ctx)),
    vscode.commands.registerCommand('mcp.stop', stop),
    vscode.commands.registerCommand('mcp.showLogs', () => output?.show(true)),
  )
}

export async function deactivate() {
  await stop()
}
