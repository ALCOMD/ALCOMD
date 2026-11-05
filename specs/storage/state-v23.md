# State23 project template catalogue

`project_templates` keys each row by owner and public template ID. The normalized declaration is a JSON object of at most 24 KiB and contains its own matching ID. The Event aggregate UUID, safe-integer revision and lock flag live outside the declaration. Locked and user-created templates use the same declaration type and validation.

`project_template_revision` assigns revisions across the catalogue in the same transaction as each actual change. A deletion advances the counter, so a template recreated under an old ID cannot match a stale revision. Repeating a creation with the same declaration returns the existing record. Updates that leave the declaration unchanged do not advance its revision.

Catalogue mutations write `project-template.changed` under the `project-template` aggregate. The event uses the internal UUID and includes the public ID in its payload. The migration retains existing event rows, identities and the sequence watermark while adding the aggregate constraint.
