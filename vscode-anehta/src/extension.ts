import * as vscode from 'vscode';
import * as path from 'path';
import * as cp from 'child_process';

// ═══════════════════════════════════════════════════════════
//  Constants
// ═══════════════════════════════════════════════════════════

const TYPES = ['int', 'string', 'str', 'number', 'bool'];

const KEYWORDS = [
    'func', 'var', 'if', 'else', 'elseif', 'for',
    'return', 'break', 'continue', 'timer', 'new',
    'true', 'false',
];

interface BuiltinInfo { signature: string; doc: string; }
const BUILTINS: Record<string, BuiltinInfo> = {
    print: { signature: 'print(value)', doc: 'Print a value to stdout. Accepts int, string, or bool.' },
    input: { signature: 'input() -> int', doc: 'Read an integer from stdin.' },
};

// ═══════════════════════════════════════════════════════════
//  Document scanner — extract symbols
// ═══════════════════════════════════════════════════════════

interface FuncInfo {
    name: string;
    params: { name: string; type: string }[];
    returnType: string;
    line: number;
    nameOffset: number;  // char offset in document
}

interface VarInfo {
    name: string;
    line: number;
    nameOffset: number;
}

interface TableFieldInfo {
    name: string;
    valueHint: string;   // e.g. "100", "\"hello\"", "{ ... }"
    isTable: boolean;     // nested table?
    nestedFields?: TableFieldInfo[];
}

interface TableVarInfo {
    name: string;
    fields: TableFieldInfo[];
    line: number;
}

interface SymbolTable {
    funcs: FuncInfo[];
    vars: VarInfo[];
    tables: TableVarInfo[];
}

/** Extract table fields from text starting at the opening '{'. */
function extractTableFields(text: string, braceStart: number): TableFieldInfo[] | null {
    let depth = 0;
    let i = braceStart;
    while (i < text.length) {
        if (text[i] === '{') depth++;
        else if (text[i] === '}') { depth--; if (depth === 0) break; }
        else if (text[i] === '"') { i++; while (i < text.length && text[i] !== '"') { if (text[i] === '\\') i++; i++; } }
        i++;
    }
    if (depth !== 0) return null;
    const braceEnd = i;
    const inner = text.substring(braceStart + 1, braceEnd);

    const fields: TableFieldInfo[] = [];
    let fd = 0, start = 0;
    for (let j = 0; j <= inner.length; j++) {
        const ch = j < inner.length ? inner[j] : ',';
        if (ch === '{') fd++;
        else if (ch === '}') fd--;
        else if (ch === '"') { j++; while (j < inner.length && inner[j] !== '"') { if (inner[j] === '\\') j++; j++; } }
        else if (ch === ',' && fd === 0) {
            const part = inner.substring(start, j).trim();
            const fm = part.match(/^([a-zA-Z_]\w*)\s*:\s*([\s\S]*)/);
            if (fm) {
                const valTrimmed = fm[2].trim();
                const isTable = valTrimmed.startsWith('{');
                let nestedFields: TableFieldInfo[] | undefined;
                if (isTable) {
                    const nestedStart = braceStart + 1 + inner.indexOf(valTrimmed, start);
                    nestedFields = extractTableFields(text, nestedStart) ?? undefined;
                }
                fields.push({
                    name: fm[1],
                    valueHint: valTrimmed.length > 30 ? valTrimmed.substring(0, 30) + '...' : valTrimmed,
                    isTable,
                    nestedFields,
                });
            }
            start = j + 1;
        }
    }
    return fields;
}

function scanDocument(doc: vscode.TextDocument): SymbolTable {
    const text = doc.getText();
    const funcs: FuncInfo[] = [];
    const vars: VarInfo[] = [];
    const tables: TableVarInfo[] = [];

    const funcRe = /\bfunc\s+([a-zA-Z_]\w*)\s*\(([^)]*)\)(?:\s*->\s*(\w+))?/g;
    let m: RegExpExecArray | null;
    while ((m = funcRe.exec(text)) !== null) {
        const params: { name: string; type: string }[] = [];
        if (m[2].trim().length > 0) {
            for (const part of m[2].split(',')) {
                const pm = part.trim().match(/^([a-zA-Z_]\w*)\s*:\s*(\w+)$/);
                if (pm) {
                    params.push({ name: pm[1], type: pm[2] });
                }
            }
        }
        funcs.push({
            name: m[1],
            params,
            returnType: m[3] || 'void',
            line: doc.positionAt(m.index).line,
            nameOffset: m.index + m[0].indexOf(m[1]),
        });
    }

    const varRe = /\bvar\s+([a-zA-Z_]\w*)/g;
    while ((m = varRe.exec(text)) !== null) {
        vars.push({
            name: m[1],
            line: doc.positionAt(m.index).line,
            nameOffset: m.index + m[0].indexOf(m[1]),
        });
    }

    // Scan table literals: var NAME = { ... }
    const varTableRe = /\bvar\s+([a-zA-Z_]\w*)\s*=\s*\{/g;
    while ((m = varTableRe.exec(text)) !== null) {
        const braceIdx = m.index + m[0].length - 1;
        const fields = extractTableFields(text, braceIdx);
        if (fields !== null) {
            tables.push({
                name: m[1],
                fields,
                line: doc.positionAt(m.index).line,
            });
        }
    }

    // Scan reassignment: NAME = { ... } (update existing table)
    const reassignRe = /\b([a-zA-Z_]\w*)\s*=\s*\{/g;
    while ((m = reassignRe.exec(text)) !== null) {
        // Skip if this is a var declaration (already handled above)
        const prefix = text.substring(Math.max(0, m.index - 10), m.index);
        if (/\bvar\s+$/.test(prefix)) continue;
        const braceIdx = m.index + m[0].length - 1;
        const fields = extractTableFields(text, braceIdx);
        if (fields !== null) {
            const existing = tables.find(t => t.name === m![1]);
            if (existing) {
                // Merge new fields into existing (reassignment may have different fields)
                for (const f of fields) {
                    if (!existing.fields.find(ef => ef.name === f.name)) {
                        existing.fields.push(f);
                    }
                }
            }
        }
    }

    // Scan field assignments: NAME.field = value
    const dotAssignRe = /\b([a-zA-Z_]\w*)\.([a-zA-Z_]\w*)\s*=/g;
    while ((m = dotAssignRe.exec(text)) !== null) {
        // Skip == comparison
        if (text[m.index + m[0].length] === '=') continue;
        const tbl = tables.find(t => t.name === m![1]);
        if (tbl && !tbl.fields.find(f => f.name === m![2])) {
            tbl.fields.push({ name: m[2], valueHint: '(assigned)', isTable: false });
        }
    }

    // Scan index assignments: NAME["field"] = value
    const idxAssignRe = /\b([a-zA-Z_]\w*)\["([^"]+)"\]\s*=/g;
    while ((m = idxAssignRe.exec(text)) !== null) {
        if (text[m.index + m[0].length] === '=') continue;
        const tbl = tables.find(t => t.name === m![1]);
        if (tbl && !tbl.fields.find(f => f.name === m![2])) {
            tbl.fields.push({ name: m[2], valueHint: '(assigned)', isTable: false });
        }
    }

    return { funcs, vars, tables };
}

// ═══════════════════════════════════════════════════════════
//  Semantic Tokens — highlight variables, params, functions
// ═══════════════════════════════════════════════════════════

const TOKEN_TYPES = [
    'function',     // 0 - function declarations & calls
    'variable',     // 1 - variable references
    'parameter',    // 2 - function parameters
    'type',         // 3 - type names
    'keyword',      // 4
    'number',       // 5
    'string',       // 6
    'comment',      // 7
    'operator',     // 8
    'property',     // 9 - table field access
];

const TOKEN_MODIFIERS = [
    'declaration',   // 0
    'definition',    // 1
    'readonly',      // 2
    'defaultLibrary', // 3
];

const legend = new vscode.SemanticTokensLegend(TOKEN_TYPES, TOKEN_MODIFIERS);

class AnehtaSemanticTokensProvider implements vscode.DocumentSemanticTokensProvider {
    provideDocumentSemanticTokens(doc: vscode.TextDocument): vscode.SemanticTokens {
        const builder = new vscode.SemanticTokensBuilder(legend);
        const text = doc.getText();
        const { funcs, vars } = scanDocument(doc);

        // Build name sets for lookup
        const funcNames = new Set(funcs.map(f => f.name));
        const builtinNames = new Set(Object.keys(BUILTINS));

        // Collect parameter names per function scope (line ranges)
        // For simplicity, we gather all param names
        const paramNames = new Set<string>();
        for (const f of funcs) {
            for (const p of f.params) {
                paramNames.add(p.name);
            }
        }

        const varNames = new Set(vars.map(v => v.name));

        // Scan every identifier in the document and classify it
        const identRe = /\b([a-zA-Z_]\w*)\b/g;
        let im: RegExpExecArray | null;
        while ((im = identRe.exec(text)) !== null) {
            const name = im[1];
            const pos = doc.positionAt(im.index);
            const line = pos.line;
            const char = pos.character;

            // Skip if inside a comment
            const lineText = doc.lineAt(line).text;
            const commentIdx = lineText.indexOf('//');
            if (commentIdx >= 0 && char >= commentIdx) continue;

            // Skip if inside a string (rough check)
            const beforeOnLine = lineText.substring(0, char);
            const quoteCount = (beforeOnLine.match(/(?<!\\)"/g) || []).length;
            if (quoteCount % 2 !== 0) continue;

            // Skip keywords and constants — handled by TextMate
            if (KEYWORDS.includes(name)) continue;
            if (TYPES.includes(name)) continue;

            // Check what's before this identifier to decide context
            const charBefore = im.index > 0 ? text[im.index - 1] : '';
            const twoBefore = im.index > 1 ? text.substring(im.index - 2, im.index) : '';

            // After 'func ' → function declaration (already handled by TextMate, but reinforce)
            const prefixCheck = text.substring(Math.max(0, im.index - 10), im.index);

            // Type annotation context (after : or ->)
            if (/(?::|->\s*)\s*$/.test(prefixCheck) && TYPES.includes(name)) {
                builder.push(line, char, name.length, 3, 0); // type
                continue;
            }

            // Function definition name
            if (/\bfunc\s+$/.test(prefixCheck)) {
                builder.push(line, char, name.length, 0, 1); // function, definition
                continue;
            }

            // Built-in function call
            if (builtinNames.has(name)) {
                const after = text.substring(im.index + name.length).match(/^\s*\(/);
                if (after) {
                    builder.push(line, char, name.length, 0, 3); // function, defaultLibrary
                    continue;
                }
            }

            // User-defined function call
            if (funcNames.has(name)) {
                const after = text.substring(im.index + name.length).match(/^\s*\(/);
                if (after) {
                    builder.push(line, char, name.length, 0, 0); // function
                    continue;
                }
            }

            // Parameter reference — check if we're inside a function that has this param
            if (paramNames.has(name)) {
                builder.push(line, char, name.length, 2, 0); // parameter
                continue;
            }

            // Variable declaration (after 'var ')
            if (/\bvar\s+$/.test(prefixCheck)) {
                builder.push(line, char, name.length, 1, 0); // variable, declaration
                continue;
            }

            // Variable reference
            if (varNames.has(name)) {
                builder.push(line, char, name.length, 1, 0); // variable
                continue;
            }

            // Unknown identifier — could be a variable we missed, still highlight as variable
            // Only if it's not a type name in a type position
            if (!TYPES.includes(name) && !KEYWORDS.includes(name)) {
                builder.push(line, char, name.length, 1, 0); // variable (fallback)
            }
        }

        return builder.build();
    }
}

// ═══════════════════════════════════════════════════════════
//  Diagnostics — call compiler and parse errors
// ═══════════════════════════════════════════════════════════

let diagnosticCollection: vscode.DiagnosticCollection;
let diagnosticTimer: NodeJS.Timeout | undefined;

function getCliPath(): string {
    return vscode.workspace.getConfiguration('anehta').get<string>('cliPath', 'anehta');
}

function runDiagnostics(doc: vscode.TextDocument) {
    if (doc.languageId !== 'anehta') return;
    if (doc.uri.scheme !== 'file') return;

    const filePath = doc.uri.fsPath;
    const cli = getCliPath();

    // Run the compiler in build mode to get errors
    cp.execFile(
        cli, ['build', filePath],
        { timeout: 10000 },
        (error, stdout, stderr) => {
            const diagnostics: vscode.Diagnostic[] = [];
            const output = (stdout || '') + '\n' + (stderr || '');

            // Parse: "Lex error at line X, column Y: message"
            // Parse: "Parse error at line X, column Y: message"
            // Parse: "Codegen error: message"
            const errRe = /(?:Lex|Parse|Codegen)\s+error(?:\s+at\s+line\s+(\d+),\s*column\s+(\d+))?:\s*(.+)/gi;
            let m: RegExpExecArray | null;
            while ((m = errRe.exec(output)) !== null) {
                const line = m[1] ? parseInt(m[1]) - 1 : 0;
                const col = m[2] ? parseInt(m[2]) - 1 : 0;
                const message = m[3].trim();

                const range = new vscode.Range(line, col, line, col + 1);
                const severity = vscode.DiagnosticSeverity.Error;
                const diag = new vscode.Diagnostic(range, message, severity);
                diag.source = 'anehta';
                diagnostics.push(diag);
            }

            diagnosticCollection.set(doc.uri, diagnostics);
        }
    );
}

function scheduleDiagnostics(doc: vscode.TextDocument) {
    if (diagnosticTimer) {
        clearTimeout(diagnosticTimer);
    }
    diagnosticTimer = setTimeout(() => runDiagnostics(doc), 500);
}

// ═══════════════════════════════════════════════════════════
//  Completion provider
// ═══════════════════════════════════════════════════════════

function getLineContext(doc: vscode.TextDocument, pos: vscode.Position): 'type' | 'statement' | 'expression' {
    const prefix = doc.lineAt(pos.line).text.substring(0, pos.character);
    if (/(?::|->\s*)\s*\w*$/.test(prefix)) return 'type';
    if (/^\s*\w*$/.test(prefix)) return 'statement';
    return 'expression';
}

/** Resolve table fields for a dot-chain like "a.b.c" */
function resolveTableChain(tables: TableVarInfo[], chain: string[]): TableFieldInfo[] | null {
    if (chain.length === 0) return null;
    const tbl = tables.find(t => t.name === chain[0]);
    if (!tbl) return null;
    let fields = tbl.fields;
    for (let i = 1; i < chain.length; i++) {
        const f = fields.find(fi => fi.name === chain[i]);
        if (!f || !f.isTable || !f.nestedFields) return null;
        fields = f.nestedFields;
    }
    return fields;
}

function makeCompletions(doc: vscode.TextDocument, pos: vscode.Position): vscode.CompletionItem[] {
    const { funcs, vars, tables } = scanDocument(doc);
    const items: vscode.CompletionItem[] = [];
    const lineText = doc.lineAt(pos.line).text;
    const prefix = lineText.substring(0, pos.character);

    // Check if we're in a dot-access context: "varName." or "varName.field1."
    const dotMatch = prefix.match(/\b([a-zA-Z_]\w*(?:\.[a-zA-Z_]\w*)*)\.(\w*)$/);
    if (dotMatch) {
        const chain = dotMatch[1].split('.');
        const partial = dotMatch[2]; // what user has typed after the last dot
        const fields = resolveTableChain(tables, chain);
        if (fields) {
            for (const f of fields) {
                const item = new vscode.CompletionItem(f.name, f.isTable
                    ? vscode.CompletionItemKind.Struct
                    : vscode.CompletionItemKind.Field);
                item.detail = f.isTable ? 'table' : f.valueHint;
                if (f.isTable && f.nestedFields) {
                    const nestedKeys = f.nestedFields.map(nf => nf.name).join(', ');
                    item.documentation = new vscode.MarkdownString(
                        `Nested table with fields: \`${nestedKeys}\``
                    );
                }
                items.push(item);
            }
            return items; // Only return field completions in dot context
        }
    }

    // Check if in ["..."] context: varName["
    const bracketMatch = prefix.match(/\b([a-zA-Z_]\w*)\["([^"]*)$/);
    if (bracketMatch) {
        const tbl = tables.find(t => t.name === bracketMatch[1]);
        if (tbl) {
            for (const f of tbl.fields) {
                const item = new vscode.CompletionItem(f.name, vscode.CompletionItemKind.Field);
                item.detail = f.isTable ? 'table' : f.valueHint;
                item.insertText = `${f.name}"]`;
                items.push(item);
            }
            return items;
        }
    }

    const context = getLineContext(doc, pos);

    if (context === 'type') {
        for (const t of TYPES) {
            const item = new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter);
            item.detail = `type ${t}`;
            items.push(item);
        }
        return items;
    }

    // Keywords
    if (context === 'statement') {
        for (const kw of KEYWORDS) {
            const item = new vscode.CompletionItem(kw, vscode.CompletionItemKind.Keyword);
            item.detail = 'keyword';
            items.push(item);
        }
    }

    // Built-in functions
    for (const [name, info] of Object.entries(BUILTINS)) {
        const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Function);
        item.detail = info.signature;
        item.documentation = new vscode.MarkdownString(info.doc);
        item.insertText = new vscode.SnippetString(`${name}($1)`);
        items.push(item);
    }

    // User functions
    for (const f of funcs) {
        const item = new vscode.CompletionItem(f.name, vscode.CompletionItemKind.Function);
        const paramStr = f.params.map(p => `${p.name}: ${p.type}`).join(', ');
        item.detail = `func ${f.name}(${paramStr}) -> ${f.returnType}`;
        if (f.params.length > 0) {
            const placeholders = f.params.map((p, i) => `\${${i + 1}:${p.name}}`).join(', ');
            item.insertText = new vscode.SnippetString(`${f.name}(${placeholders})`);
        } else {
            item.insertText = new vscode.SnippetString(`${f.name}()`);
        }
        items.push(item);
    }

    // Variables (deduplicated) — mark table vars with field info
    const seen = new Set<string>();
    for (const v of vars) {
        if (seen.has(v.name)) continue;
        seen.add(v.name);
        const item = new vscode.CompletionItem(v.name, vscode.CompletionItemKind.Variable);
        const tbl = tables.find(t => t.name === v.name);
        if (tbl) {
            const fieldNames = tbl.fields.map(f => f.name).join(', ');
            item.detail = `table { ${fieldNames} }`;
            item.documentation = new vscode.MarkdownString(
                `Table with fields: ${tbl.fields.map(f => `\`${f.name}\`: ${f.valueHint}`).join(', ')}`
            );
        } else {
            item.detail = `var ${v.name}`;
        }
        items.push(item);
    }

    return items;
}

// ═══════════════════════════════════════════════════════════
//  Signature help
// ═══════════════════════════════════════════════════════════

class AnehtaSignatureHelpProvider implements vscode.SignatureHelpProvider {
    provideSignatureHelp(doc: vscode.TextDocument, pos: vscode.Position): vscode.SignatureHelp | null {
        const prefix = doc.lineAt(pos.line).text.substring(0, pos.character);
        const callMatch = /\b([a-zA-Z_]\w*)\s*\(([^)]*)$/.exec(prefix);
        if (!callMatch) return null;

        const funcName = callMatch[1];
        const argsTyped = callMatch[2];
        const activeParam = (argsTyped.match(/,/g) || []).length;

        if (BUILTINS[funcName]) {
            const info = BUILTINS[funcName];
            const sig = new vscode.SignatureInformation(info.signature, info.doc);
            const help = new vscode.SignatureHelp();
            help.signatures = [sig];
            help.activeSignature = 0;
            help.activeParameter = activeParam;
            return help;
        }

        const { funcs } = scanDocument(doc);
        const found = funcs.find(f => f.name === funcName);
        if (!found) return null;

        const paramStr = found.params.map(p => `${p.name}: ${p.type}`).join(', ');
        const sigLabel = `func ${found.name}(${paramStr}) -> ${found.returnType}`;
        const sig = new vscode.SignatureInformation(sigLabel);
        sig.parameters = found.params.map(p =>
            new vscode.ParameterInformation(`${p.name}: ${p.type}`)
        );

        const help = new vscode.SignatureHelp();
        help.signatures = [sig];
        help.activeSignature = 0;
        help.activeParameter = activeParam;
        return help;
    }
}

// ═══════════════════════════════════════════════════════════
//  Hover provider
// ═══════════════════════════════════════════════════════════

class AnehtaHoverProvider implements vscode.HoverProvider {
    provideHover(doc: vscode.TextDocument, pos: vscode.Position): vscode.Hover | null {
        const wordRange = doc.getWordRangeAtPosition(pos, /[a-zA-Z_]\w*/);
        if (!wordRange) return null;
        const word = doc.getText(wordRange);

        const { funcs, tables } = scanDocument(doc);

        // Check if hovering over a table field: "varName.field"
        const lineText = doc.lineAt(pos.line).text;
        const charStart = wordRange.start.character;
        const beforeWord = lineText.substring(0, charStart);
        const dotChainMatch = beforeWord.match(/\b([a-zA-Z_]\w*(?:\.[a-zA-Z_]\w*)*)\.$/);
        if (dotChainMatch) {
            const chain = dotChainMatch[1].split('.');
            const fields = resolveTableChain(tables, chain);
            if (fields) {
                const field = fields.find(f => f.name === word);
                if (field) {
                    const parentName = dotChainMatch[1];
                    if (field.isTable && field.nestedFields) {
                        const nestedKeys = field.nestedFields.map(nf =>
                            `  ${nf.name}: ${nf.valueHint}`
                        ).join('\n');
                        return new vscode.Hover(new vscode.MarkdownString(
                            `\`\`\`anehta\n${parentName}.${word} = {\n${nestedKeys}\n}\n\`\`\`\n\n*nested table*`
                        ));
                    }
                    return new vscode.Hover(new vscode.MarkdownString(
                        `\`\`\`anehta\n${parentName}.${word} = ${field.valueHint}\n\`\`\``
                    ));
                }
            }
        }

        // Check if hovering over a table variable name
        const tbl = tables.find(t => t.name === word);
        if (tbl) {
            const fieldLines = tbl.fields.map(f => {
                if (f.isTable) return `  ${f.name}: { ... }`;
                return `  ${f.name}: ${f.valueHint}`;
            }).join('\n');
            return new vscode.Hover(new vscode.MarkdownString(
                `\`\`\`anehta\nvar ${word} = {\n${fieldLines}\n}\n\`\`\`\n\n*table — line ${tbl.line + 1}*`
            ));
        }

        if (BUILTINS[word]) {
            const info = BUILTINS[word];
            return new vscode.Hover(new vscode.MarkdownString(
                `\`\`\`anehta\n${info.signature}\n\`\`\`\n\n${info.doc}`
            ));
        }

        const found = funcs.find(f => f.name === word);
        if (found) {
            const paramStr = found.params.map(p => `${p.name}: ${p.type}`).join(', ');
            return new vscode.Hover(new vscode.MarkdownString(
                `\`\`\`anehta\nfunc ${found.name}(${paramStr}) -> ${found.returnType}\n\`\`\`\n\n*Line ${found.line + 1}*`
            ));
        }

        const kwDocs: Record<string, string> = {
            func: '`func name(params) -> type { ... }`',
            var: '`var name = value` or `var name: type = value`',
            timer: '`timer { ... }` — auto-measures and prints elapsed time',
            if: '`if (condition) { ... }`',
            elseif: '`elseif (condition) { ... }`',
            else: '`else { ... }`',
            for: '`for (init; cond; step) { ... }`',
            return: 'Return a value from a function',
            break: 'Exit the current loop',
            continue: 'Skip to the next loop iteration',
        };
        if (kwDocs[word]) {
            return new vscode.Hover(new vscode.MarkdownString(kwDocs[word]));
        }

        return null;
    }
}

// ═══════════════════════════════════════════════════════════
//  Build / Run commands
// ═══════════════════════════════════════════════════════════

function getActiveAhFile(): string | undefined {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor.');
        return undefined;
    }
    if (!editor.document.fileName.endsWith('.ah')) {
        vscode.window.showErrorMessage('Current file is not an .ah file.');
        return undefined;
    }
    if (editor.document.isDirty) {
        editor.document.save();
    }
    return editor.document.fileName;
}

let anehtaTerminal: vscode.Terminal | undefined;

function getTerminal(): vscode.Terminal {
    if (anehtaTerminal && !anehtaTerminal.exitStatus) {
        return anehtaTerminal;
    }
    anehtaTerminal = vscode.window.createTerminal('Anehta');
    vscode.window.onDidCloseTerminal(t => {
        if (t === anehtaTerminal) anehtaTerminal = undefined;
    });
    return anehtaTerminal;
}

function runInTerminal(cli: string, args: string[]) {
    const terminal = getTerminal();
    terminal.show(true);
    // Use & operator for PowerShell compatibility
    const quoted = args.map(a => `"${a}"`).join(' ');
    terminal.sendText(`& "${cli}" ${quoted}`);
}

// ═══════════════════════════════════════════════════════════
//  Activate
// ═══════════════════════════════════════════════════════════

export function activate(ctx: vscode.ExtensionContext) {
    const selector: vscode.DocumentSelector = { language: 'anehta', scheme: 'file' };

    // ── Diagnostics ────────────────────────────────────────
    diagnosticCollection = vscode.languages.createDiagnosticCollection('anehta');
    ctx.subscriptions.push(diagnosticCollection);

    // Run diagnostics on open / save / change
    ctx.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument(doc => {
            if (doc.languageId === 'anehta') runDiagnostics(doc);
        }),
        vscode.workspace.onDidSaveTextDocument(doc => {
            if (doc.languageId === 'anehta') runDiagnostics(doc);
        }),
        vscode.workspace.onDidChangeTextDocument(e => {
            if (e.document.languageId === 'anehta') scheduleDiagnostics(e.document);
        }),
    );

    // Run diagnostics for already-open .ah files
    for (const doc of vscode.workspace.textDocuments) {
        if (doc.languageId === 'anehta') runDiagnostics(doc);
    }

    // ── Semantic tokens ────────────────────────────────────
    ctx.subscriptions.push(
        vscode.languages.registerDocumentSemanticTokensProvider(
            selector,
            new AnehtaSemanticTokensProvider(),
            legend,
        ),
    );

    // ── Completion ─────────────────────────────────────────
    ctx.subscriptions.push(
        vscode.languages.registerCompletionItemProvider(
            selector,
            { provideCompletionItems: makeCompletions },
            '.', ':', '>', '"',
        ),
    );

    // ── Signature help ─────────────────────────────────────
    ctx.subscriptions.push(
        vscode.languages.registerSignatureHelpProvider(
            selector,
            new AnehtaSignatureHelpProvider(),
            '(', ',',
        ),
    );

    // ── Hover ──────────────────────────────────────────────
    ctx.subscriptions.push(
        vscode.languages.registerHoverProvider(selector, new AnehtaHoverProvider()),
    );

    // ── Commands ───────────────────────────────────────────
    ctx.subscriptions.push(
        vscode.commands.registerCommand('anehta.build', () => {
            const f = getActiveAhFile();
            if (f) runInTerminal(getCliPath(), ['build', f]);
        }),
        vscode.commands.registerCommand('anehta.run', () => {
            const f = getActiveAhFile();
            if (f) runInTerminal(getCliPath(), ['run', f]);
        }),
        vscode.commands.registerCommand('anehta.buildAndRun', () => {
            const f = getActiveAhFile();
            if (f) runInTerminal(getCliPath(), ['run', f]);
        }),
    );
}

export function deactivate() {
    if (diagnosticTimer) clearTimeout(diagnosticTimer);
}
