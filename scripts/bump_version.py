#!/usr/bin/env python3
import re
import sys
import subprocess
from pathlib import Path

def bump_version(pr_num="0", message="chore: release update"):
    cargo_toml = Path("Cargo.toml")
    content = cargo_toml.read_text(encoding="utf-8")

    # Match version in [package]
    pattern = r'(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"'
    match = re.search(pattern, content)
    if not match:
        print("Error: Could not find version in Cargo.toml", file=sys.stderr)
        sys.exit(1)

    major, minor, patch = map(int, match.groups())
    next_patch = patch + 1
    new_version = f"{major}.{minor}.{next_patch}"

    # Update Cargo.toml
    updated_content = re.sub(
        pattern,
        f'version = "{new_version}"',
        content,
        count=1
    )
    cargo_toml.write_text(updated_content, encoding="utf-8")
    print(f"==> Bumped Cargo.toml version: {major}.{minor}.{patch} -> {new_version}")

    # Update Cargo.lock using cargo check
    subprocess.run(["cargo", "check", "--quiet"], check=True)
    print("==> Synchronized Cargo.lock")

    # Update CHANGELOG.md
    changelog_path = Path("CHANGELOG.md")
    if changelog_path.exists():
        cl_text = changelog_path.read_text(encoding="utf-8")
        pr_suffix = f" — PR #{pr_num}" if pr_num and pr_num != "0" else ""
        header = f"## [{new_version}]{pr_suffix}\n- {message}\n"

        # Insert after the top title/description
        parts = cl_text.split("## [", 1)
        if len(parts) == 2:
            new_cl = parts[0] + header + "\n## [" + parts[1]
        else:
            new_cl = cl_text + "\n" + header

        changelog_path.write_text(new_cl, encoding="utf-8")
        print(f"==> Added changelog entry for [{new_version}]")

if __name__ == "__main__":
    pr = sys.argv[1] if len(sys.argv) > 1 else "0"
    msg = sys.argv[2] if len(sys.argv) > 2 else "chore: release update"
    bump_version(pr, msg)
