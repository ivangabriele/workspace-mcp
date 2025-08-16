#!/usr/bin/env bun

import { B } from 'bhala'

/**
 * Run a given npm script in every workspace discovered from the root package.json,
 * similar to `yarn workspaces foreach run <script>`.
 *
 * Features:
 * - Topological execution order based on internal workspace deps (default on).
 * - Concurrency control with --parallel N (default 1).
 * - Include/Exclude filters via globs.
 * - Prefixed streaming output (one line per package with name prefix).
 * - Dry-run mode.
 * - Optionally ignore packages missing the script.
 *
 * No external dependencies. Requires Bun (uses Bun.Glob and Bun.spawn).
 *
 * Example:
 *   bun run tools/workspaces-run.ts --script build --parallel 4
 */

type Json = Record<string, unknown>

type PkgJson = {
  name?: string
  version?: string
  private?: boolean
  scripts?: Record<string, string>
  workspaces?: string[] | { packages?: string[] }
  dependencies?: Record<string, string>
  devDependencies?: Record<string, string>
  optionalDependencies?: Record<string, string>
  peerDependencies?: Record<string, string>
}

type Workspace = {
  name: string
  dir: string
  scripts: Record<string, string>
  depsInternal: Set<string>
}

type CliArgs = {
  script: string
  parallel: number
  include: string[]
  exclude: string[]
  topo: boolean
  ignoreMissing: boolean
  dryRun: boolean
}

const ROOT = process.cwd()

/**
 * Parse CLI flags.
 */
function parseArgs(argv: string[]): CliArgs {
  const args: CliArgs = {
    dryRun: false,
    exclude: [],
    ignoreMissing: false,
    include: [],
    parallel: 1,
    script: '',
    topo: true,
  }

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--script' || a === '-s') {
      args.script = String(argv[++i] ?? '')
    } else if (a === '--parallel' || a === '-p') {
      args.parallel = Math.max(1, Number(argv[++i] ?? '1'))
    } else if (a === '--include') {
      args.include.push(String(argv[++i] ?? ''))
    } else if (a === '--exclude') {
      args.exclude.push(String(argv[++i] ?? ''))
    } else if (a === '--no-topo') {
      args.topo = false
    } else if (a === '--ignore-missing') {
      args.ignoreMissing = true
    } else if (a === '--dry-run') {
      args.dryRun = true
    }
  }

  if (!args.script) {
    B.error('Error: --script <name> is required.')
    process.exit(1)
  }
  return args
}

/**
 * Read and parse a JSON file.
 */
async function readJson<T extends Json>(path: string): Promise<T> {
  const text = await Bun.file(path).text()
  return JSON.parse(text) as T
}

/**
 * Resolve workspaces globs from the root package.json.
 */
async function resolveWorkspaceDirs(): Promise<string[]> {
  const pkg = await readJson<PkgJson>(`${ROOT}/package.json`)
  const workspaces: string[] = Array.isArray(pkg.workspaces)
    ? (pkg.workspaces as string[])
    : (pkg.workspaces?.packages ?? [])

  return workspaces.map(workspace => `${ROOT}/${workspace}`)
}

/**
 * Load workspace package.json and collect internal deps.
 */
async function loadWorkspaces(dirs: string[]): Promise<Workspace[]> {
  const pkgs: Workspace[] = []
  for (const dir of dirs) {
    const pkgPath = `${dir}/package.json`
    try {
      const pkg = await readJson<PkgJson>(pkgPath)
      if (!pkg.name) continue

      pkgs.push({
        depsInternal: new Set<string>(), // fill later once we know all names
        dir,
        name: pkg.name,
        scripts: pkg.scripts ?? {},
      })
    } catch {
      // skip non-packages
    }
  }

  const names = new Set(pkgs.map(p => p.name))
  // Fill internal deps using deps/dev/optional (ignore peer for ordering)
  for (const ws of pkgs) {
    const pkg = await readJson<PkgJson>(`${ws.dir}/package.json`)
    const depMaps = [pkg.dependencies ?? {}, pkg.devDependencies ?? {}, pkg.optionalDependencies ?? {}]
    for (const depMap of depMaps) {
      for (const depName of Object.keys(depMap)) {
        if (names.has(depName)) ws.depsInternal.add(depName)
      }
    }
  }

  return pkgs
}

/**
 * Filter workspaces by include/exclude globs.
 */
function filterWorkspaces(all: Workspace[], include: string[], exclude: string[]): Workspace[] {
  const includeGlobs = include.map(p => new Bun.Glob(p))
  const excludeGlobs = exclude.map(p => new Bun.Glob(p))

  function matchAny(globs: Bun.Glob[], str: string): boolean {
    return globs.some(g => g.match(str))
  }

  const byPath = (w: Workspace) => w.dir.replace(`${ROOT}/`, '')

  let result = all
  if (includeGlobs.length) {
    result = result.filter(w => matchAny(includeGlobs, w.name) || matchAny(includeGlobs, byPath(w)))
  }
  if (excludeGlobs.length) {
    result = result.filter(w => !(matchAny(excludeGlobs, w.name) || matchAny(excludeGlobs, byPath(w))))
  }
  return result.sort((a, b) => a.name.localeCompare(b.name))
}

/**
 * Topologically sort workspaces by internal deps.
 * Kahn's algorithm; if cycles exist, falls back to partial order + alpha.
 */
function topoSort(workspaces: Workspace[]): Workspace[] {
  const nameToWs = new Map(workspaces.map(w => [w.name, w]))
  const inDegree = new Map<string, number>()
  const graph = new Map<string, Set<string>>()

  for (const w of workspaces) {
    inDegree.set(w.name, inDegree.get(w.name) ?? 0)
    for (const dep of w.depsInternal) {
      if (!nameToWs.has(dep)) continue
      const set = graph.get(dep) ?? new Set<string>()
      set.add(w.name)
      graph.set(dep, set)
      inDegree.set(w.name, (inDegree.get(w.name) ?? 0) + 1)
      inDegree.set(dep, inDegree.get(dep) ?? 0)
    }
  }

  const queue: string[] = []
  for (const [name, deg] of inDegree) {
    if (deg === 0) queue.push(name)
  }
  queue.sort()

  const out: string[] = []
  while (queue.length) {
    const n = queue.shift()!
    out.push(n)
    for (const m of graph.get(n) ?? []) {
      const d = (inDegree.get(m) ?? 0) - 1
      inDegree.set(m, d)
      if (d === 0) queue.push(m)
    }
    queue.sort()
  }

  // If cycle, append remaining in alpha order
  const remaining = workspaces
    .map(w => w.name)
    .filter(n => !out.includes(n))
    .sort()
  const order = [...out, ...remaining]

  return order.map(n => nameToWs.get(n)!).filter(Boolean)
}

/**
 * Spawn "bun run <script>" in a workspace, prefixing output with the package name.
 */
async function runScriptInWorkspace(ws: Workspace, script: string): Promise<number> {
  const child = Bun.spawn(['bun', 'run', script], {
    cwd: ws.dir,
    env: process.env,
    stderr: 'pipe',
    stdout: 'pipe',
  })

  const prefix = `[${ws.name}] `
  const decoder = new TextDecoder()

  const pump = async (readable: ReadableStream<Uint8Array>, write: (s: string) => void) => {
    let buf = ''
    for await (const chunk of readable) {
      buf += decoder.decode(chunk)
      let idx: number
      while ((idx = buf.indexOf('\n')) !== -1) {
        const line = buf.slice(0, idx + 1)
        write(prefix + line)
        buf = buf.slice(idx + 1)
      }
    }
    if (buf.length) write(prefix + buf + '\n')
  }

  const stdoutTask = pump(child.stdout!, s => process.stdout.write(s))
  const stderrTask = pump(child.stderr!, s => process.stderr.write(s))

  const status = await child.exited
  await Promise.all([stdoutTask, stderrTask])
  return status
}

/**
 * Simple concurrency runner.
 */
async function runWithConcurrency<T>(items: T[], limit: number, fn: (x: T) => Promise<number>): Promise<number> {
  let idx = 0
  let failures = 0

  async function worker(): Promise<void> {
    while (idx < items.length) {
      const i = idx++
      const code = await fn(items[i])
      if (code !== 0) failures++
    }
  }

  const workers = Array.from({ length: Math.min(limit, items.length) }, () => worker())
  await Promise.all(workers)
  return failures
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv.slice(2))
  const allDirs = await resolveWorkspaceDirs()
  const all = await loadWorkspaces(allDirs)
  console.log({
    allDirs: allDirs,
    dryRun: args.dryRun,
    exclude: args.exclude,
    ignoreMissing: args.ignoreMissing,
    include: args.include,
    parallel: args.parallel,
    script: args.script,
    topo: args.topo,
    workspacesCount: all.length,
  })

  let selected = filterWorkspaces(all, args.include, args.exclude)

  if (args.topo) {
    selected = topoSort(selected)
  }

  if (!selected.length) {
    B.error('No workspaces matched.')
    process.exit(1)
  }

  const hasScript = (w: Workspace) => Boolean(w.scripts[args.script])

  const final = args.ignoreMissing ? selected.filter(hasScript) : selected
  if (!final.length) {
    B.error(`No workspaces contain script "${args.script}".`)
    process.exit(1)
  }

  if (args.dryRun) {
    B.info('Dry run. Would execute in order:')
    for (const w of final) {
      const flag = hasScript(w) ? '' : ' (missing script)'
      B.info(` - ${w.name}${flag}`)
    }
    return
  }

  // Serial topological batches when topo=true && parallel=1; otherwise just concurrency queue.
  const failures = await runWithConcurrency(final, args.parallel, async w => {
    if (!hasScript(w)) {
      B.error(`[${w.name}] Script "${args.script}" not found.`)
      return 1
    }
    B.info(`[${w.name}] Running "${args.script}"…`)
    const code = await runScriptInWorkspace(w, args.script)
    if (code === 0) {
      B.info(`[${w.name}] ✔ Done`)
    } else {
      B.error(`[${w.name}] ✖ Failed with exit code ${code}`)
    }
    return code
  })

  if (failures > 0) {
    process.exit(1)
  }
}

try {
  await main()
} catch (err) {
  B.error(err)
  process.exit(1)
}
