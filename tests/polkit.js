// Evaluates packaging/polkit/50-creed.rules with a stand-in `polkit` object, the way polkitd
// calls it, and checks it grants exactly what it should. Run: node tests/polkit.js
const fs = require("fs");
const vm = require("vm");
const src = fs.readFileSync(__dirname + "/../packaging/polkit/50-creed.rules", "utf8");
const rules = [];
const polkit = { Result: { YES: "yes", NO: "no", AUTH_ADMIN: "auth_admin" }, addRule: (f) => rules.push(f), log: () => {} };
vm.runInNewContext(src, { polkit });
const decide = (program, s) => {
  const action = { id: "org.freedesktop.policykit.exec", lookup: (k) => (k === "program" ? program : undefined) };
  const subject = { local: s.local, active: s.active, isInGroup: (g) => s.groups.includes(g) };
  for (const r of rules) { const v = r(action, subject); if (v !== undefined) return v; }
  return "default (ask for a password)";
};
const helper = "/usr/lib/creed/creed-helper";
const cases = [
  ["admin at the seat runs the helper", helper, { local: true, active: true, groups: ["wheel"] }, "yes"],
  ["non-admin is not let through", helper, { local: true, active: true, groups: ["users"] }, "default (ask for a password)"],
  ["remote (SSH) admin is not let through", helper, { local: false, active: true, groups: ["wheel"] }, "default (ask for a password)"],
  ["inactive session is not let through", helper, { local: true, active: false, groups: ["wheel"] }, "default (ask for a password)"],
  ["any other program is untouched", "/usr/bin/bash", { local: true, active: true, groups: ["wheel"] }, "default (ask for a password)"],
  ["look-alike path is untouched", "/usr/lib/creed/creed-helper.evil", { local: true, active: true, groups: ["wheel"] }, "default (ask for a password)"],
];
let fail = 0;
for (const [name, prog, subj, want] of cases) {
  const got = decide(prog, subj);
  console.log((got === want ? "✓ " : "✗ ") + name + (got === want ? "" : ` (got ${got})`));
  if (got !== want) fail = 1;
}
// A different action id must never match either.
const other = rules.map((r) => r({ id: "org.freedesktop.systemd1.manage-units", lookup: () => helper }, { local: true, active: true, isInGroup: () => true }));
const ok = other.every((v) => v === undefined);
console.log((ok ? "✓ " : "✗ ") + "other polkit actions are untouched");
process.exit(fail || !ok ? 1 : 0);
