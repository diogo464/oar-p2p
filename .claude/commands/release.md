---
name: release
description: Create a new GitHub release by tagging and pushing
---

Create a new GitHub release for version {{version}}.

Steps:
1. First check that we're on the main branch and that it's up to date
2. Update the version in Cargo.toml from its current value to {{version}} (without the 'v' prefix)
3. Commit the version change with message: "chore: bump version to {{version}}"
4. Create an annotated git tag with: git tag -a v{{version}} -m "Release v{{version}}"
5. Check available remotes with: git remote -v
6. Push the commits and tag to all remotes (e.g., if you have both 'origin' and 'github', push to both)

This will trigger the GitHub Actions workflow that automatically builds and releases the binaries.

Important:
- The version should be provided without the 'v' prefix (e.g., "1.0.0" not "v1.0.0")
- Make sure all changes are committed before running this command
- The GitHub Actions workflow will handle building binaries for all platforms and creating the release
- Run `cargo build` after updating the version in `Cargo.toml` so the `Cargo.lock` gets updated
