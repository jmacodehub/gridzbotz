#!/usr/bin/env python3
"""
PR #76 — Add #[serde(deny_unknown_fields)] to all config structs.
Run from repo root on branch fix/deny-unknown-fields.

Usage:
    git checkout fix/deny-unknown-fields
    python3 scripts/apply_deny_unknown_fields.py
    git diff                          # verify
    cargo test                        # confirm all tests pass
    git add -A
    git commit -m "fix(config): add serde(deny_unknown_fields) to all config structs"
    git push origin fix/deny-unknown-fields
"""

import re
import sys

FILES = [
    "src/config/mod.rs",
    "src/config/fees.rs",
]

# Pattern: #[derive(...Deserialize...)] immediately followed by pub struct
PATTERN = re.compile(
    r'(#\[derive\([^\]]*Deserialize[^\]]*\)\])\n(pub struct )'
)
REPLACEMENT = r'\1\n#[serde(deny_unknown_fields)]\n\2'

total = 0
for filepath in FILES:
    try:
        with open(filepath, "r") as f:
            original = f.read()
    except FileNotFoundError:
        print(f"ERROR: {filepath} not found. Run from repo root.")
        sys.exit(1)

    modified, count = PATTERN.subn(REPLACEMENT, original)
    if count == 0:
        print(f"  \u26a0\ufe0f  {filepath}: no changes (already applied?)")
    else:
        with open(filepath, "w") as f:
            f.write(modified)
        print(f"  \u2705 {filepath}: {count} struct(s) updated")
        total += count

print(f"\n{'='*60}")
print(f"  Total: {total} structs got #[serde(deny_unknown_fields)]")
print(f"{'='*60}")
print(f"\nNext steps:")
print(f"  git diff                  # review changes")
print(f"  cargo test                # verify all tests pass")
print(f"  git add -A && git commit  # use commit message from docstring")
print(f"  git push origin fix/deny-unknown-fields")
