export type Stmt = { from: number; to: number; sql: string };

/** Splits on `;` outside quotes/backticks/comments. */
export function splitStatements(text: string): Stmt[] {
  const out: Stmt[] = [];
  let start = 0;
  let i = 0;
  const push = (end: number) => {
    const raw = text.slice(start, end);
    const sql = raw.trim();
    if (sql) {
      const lead = raw.length - raw.trimStart().length;
      out.push({ from: start + lead, to: start + lead + sql.length, sql });
    }
  };
  while (i < text.length) {
    const c = text[i];
    const n = text[i + 1];
    if (c === "'" || c === '"' || c === "`") {
      i++;
      while (i < text.length && text[i] !== c) i += text[i] === "\\" && c !== "`" ? 2 : 1;
      i++;
    } else if ((c === "-" && n === "-") || c === "#") {
      while (i < text.length && text[i] !== "\n") i++;
    } else if (c === "/" && n === "*") {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 2;
    } else if (c === ";") {
      push(i);
      start = ++i;
    } else i++;
  }
  push(text.length);
  return out;
}

/** Statement containing `pos`, else the nearest one before it, else the first after. */
export function statementAt(text: string, pos: number): Stmt | null {
  const all = splitStatements(text);
  // cursor right after a `;` (same line) still means the statement it ends
  let best: Stmt | null = null;
  for (const s of all) {
    if (s.from <= pos) best = s;
    if (s.from <= pos && pos <= s.to + 1) return s;
  }
  return best ?? all[0] ?? null;
}
