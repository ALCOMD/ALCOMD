# v3 migration-specific instructions

- Never delete an artifact whose `confirmed` value is false.
- Never infer ownership from a similar name alone.
- Never modify user projects during inventory or export.
- Build and validate v4 state before entering the irreversible commit phase.
- Legacy readers must stay under `migrations/v3/`.
- Fixtures must contain no real tokens, private project names, personal paths, or signing material.
- Every cleanup action needs a rollback/commit-stage classification and an automated residue test.
