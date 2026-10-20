// Compile the same Svelte runes used by the app for Node's built-in test runner.
// No browser or additional test framework is required.
import { readFile, access } from 'node:fs/promises';
import ts from 'typescript';
import { compileModule } from 'svelte/compiler';

export async function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith('.')) {
    for (const suffix of ['.ts', '/index.ts']) {
      const url = new URL(`${specifier}${suffix}`, context.parentURL);
      try {
        await access(url);
        return { url: url.href, shortCircuit: true };
      } catch {
        // Try the next candidate.
      }
    }
  }
  return nextResolve(specifier, context);
}

export async function load(url, context, nextLoad) {
  if (!url.startsWith('file:') || !url.endsWith('.ts')) return nextLoad(url, context);
  const input = await readFile(new URL(url), 'utf8');
  let source = ts.transpileModule(input, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  if (url.endsWith('.svelte.ts')) {
    source = compileModule(source, { filename: new URL(url).pathname, generate: 'client' }).js.code;
  }
  return { format: 'module', source, shortCircuit: true };
}
