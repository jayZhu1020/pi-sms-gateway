---
name: pi-sms-pr-workflow
description: Apply the user's branch and pull-request workflow when changing files in jayZhu1020/pi-sms-gateway, including the local sim-card-holder project. Does not apply to unrelated repositories or read-only checks.
---

# Pi SMS gateway pull-request workflow

The user reviews and merges all changes to this repository.

- Before editing, inspect the working tree, current branch, and origin. Preserve unrelated user changes.
- For each new logical change, fetch origin and create a descriptive `codex/` branch from current `origin/main`. Continue an existing task branch when updating that same unmerged PR. Never commit or push changes directly to main.
- Keep the change within the user's requested scope. Run relevant checks, inspect the diff, and commit the intended files. Do not include credentials, private keys, SMS contents, or local runtime data.
- Push the task branch to origin and create a GitHub pull request targeting main. The user's standing instruction authorizes branch pushes and PR creation for requested work in this repository.
- Explain the problem, resulting behavior, validation, and material limitations in the PR. Use a draft if work is incomplete or blocked; otherwise request review with a normal PR.
- If a PR for the branch already exists, update it instead of opening a duplicate. Attach the created or actively updated PR to the Codex chat when the attachment tool is available.
- Leave review and merge to the user. Never merge, enable auto-merge, or bypass branch protection. Do not force-push without explicit authorization.
- If authentication or permissions block publishing, preserve the local commit and report the exact blocker. Do not claim a PR exists until creation is confirmed.

Read-only troubleshooting of the Pi does not require a branch. Save requested reusable code and documentation in this repository through the workflow above.
