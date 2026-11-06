import { test } from "node:test";
import assert from "node:assert/strict";
import { splitStatements, statementAt } from "./split.ts";

test("splits ignoring quotes and comments", () => {
  const t = "select ';' a; -- x;y\nselect `a;b` from t /* ; */;\n# c;\nselect \"x\\\";\"";
  assert.deepEqual(
    splitStatements(t).map((s) => s.sql),
    ["select ';' a", "-- x;y\nselect `a;b` from t /* ; */", "# c;\nselect \"x\\\";\""],
  );
});

test("statement at cursor", () => {
  const t = "select 1;\n\nselect 2;\nselect 3";
  assert.equal(statementAt(t, 3)?.sql, "select 1");
  assert.equal(statementAt(t, 9)?.sql, "select 1"); // right after ';'
  assert.equal(statementAt(t, 10)?.sql, "select 1"); // blank line -> previous
  assert.equal(statementAt(t, 14)?.sql, "select 2");
  assert.equal(statementAt(t, t.length)?.sql, "select 3");
  assert.equal(statementAt("", 0), null);
});
