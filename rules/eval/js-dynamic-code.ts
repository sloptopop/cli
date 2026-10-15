// Test cases for sloptopop.eval.js-dynamic-code (semgrep --test format).

export function run(expression: string, body: string) {
  // ruleid: sloptopop.eval.js-dynamic-code
  const value = eval(expression);

  // ruleid: sloptopop.eval.js-dynamic-code
  const fn = new Function("a", "b", body);

  // ruleid: sloptopop.eval.js-dynamic-code
  const g = Function("return " + expression);

  // ruleid: sloptopop.eval.js-dynamic-code
  window.eval(`compute(${expression})`);

  // ok: sloptopop.eval.js-dynamic-code
  const fixed = eval("1 + 1");

  // ok: sloptopop.eval.js-dynamic-code
  const constant = new Function("a", "b", "return a + b");

  // ok: sloptopop.eval.js-dynamic-code
  const parsed = JSON.parse(body);

  return [value, fn, g, fixed, constant, parsed];
}
