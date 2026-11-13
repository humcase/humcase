---
name: deliver-results
description: Closes the loop on work tracked in Humcase. Sets a case to resolved only after verifying it, with a report of what was done, how it was verified and what remains; posts progress reports without changing the status; and handles the reworks the person asks for (read their reason, fix, verify, resolve again), never deciding on its own result. Use whenever an agent starts, finishes, pauses or reports on a Humcase case or task, when the brief lists a rework or a result waiting for a decision, or when the user says something is done, works, is wrong, needs changes or should be redone or closed.
---

# Deliver results in Humcase

In Humcase, finishing a case is a declaration, not a proof. The agent delivers a **result**; the
person reviews it and decides. The loop only works if every result is verified, every report is
honest, and only the person decides.

**Vocabulary.** A **case** is what the tools call a note; a **sub-case** lives inside a case
(`parent_id`); its **nature** is `note_type`; its **report** is `resolution_comment`. Setting a
case to `resolved` delivers a **result**; the person's **decision** then accepts it or asks for a
**rework**, with a reason.

**Tools.** They belong to the Humcase MCP server and are written here `mcp__humcase__<tool>`; when
the server comes from this plugin, Claude Code names them `mcp__plugin_humcase_humcase__<tool>`.
Without MCP, run the same tool from a terminal: `Humcase.exe call <tool> --args '<json>'`
(`Humcase.exe tools` lists them). A tool missing from your list belongs to a newer Humcase: use
the fallback given next to it.

## The loop

```text
open → in_progress (the agent starts) → resolved + report (a result, waiting for the person)
  → the person accepts it, or
  → the person asks for a rework, with a reason → open again → in_progress → resolved again
```

`mcp__humcase__get_note` shows where a case stands in `review.state`: `none`, `to_review`
(a result waits for the person), `rework_requested`, `accepted`, or `accepted_changed` (accepted,
then changed since). `review.rework_count` counts the reworks; `review.last_decision` holds the
person's latest decision, the `revision` it reviewed and, for a rework, the reason in `comment`.

No tool records a decision, and none should be simulated: never write that a result is
accepted, never resolve a case again just to clear a rework, never set a status the work does
not support. The person accepts or asks for a rework in the Humcase interface.

## Start

Before modifying anything: `mcp__humcase__update_note_status`
`{"note_id": "<id>", "status": "in_progress"}`. Read the answer (a newer Humcase returns
`{id, status, resolution_comment, revision}`; a refusal comes back as a tool error). Never announce a status whose
write failed.

## Report progress without changing the status

When work pauses, a step lands or a blocker appears, leave a report and keep the status:
`mcp__humcase__update_note_status` with `note_id` and `resolution_comment` but no `status`
(an older Humcase requires `status`: pass the current one, or use `Humcase.exe report`). Read the current report first
(`mcp__humcase__get_note`) and pass it as `expected_resolution_comment` (an empty string when
there is none): a concurrent edit then refuses the write instead of being overwritten. The new
report replaces the previous one, so rewrite it whole: what is done, what is in progress, what
blocks. To only add a few lines, pass `"append": true` instead: they go after the stored report.
Without either, Humcase refuses to replace an existing report.

## Resolve: verify first, then report

```text
- [ ] The requested behavior is verified (run, test, inspect, reproduce)
- [ ] The report says Done / Verified / Limits
- [ ] Open sub-cases are resolved or named in Limits
- [ ] The write succeeded
```

1. **Verify the requested behavior**, not only that the work compiles or looks finished. For an
   issue, reproduce it before the fix when possible and confirm it is gone after. For a
   visible change, look at it. For a document or a decision, reread it against the request.
2. **Write the report** in the user's language, with three parts (headings translated):
   - **Done**: what changed, where (files, pages, people contacted), with names and numbers.
   - **Verified**: how, and what came out (command and result, scenario followed, figures).
   - **Limits**: what remains, what could not be verified, known risks. Write "none" only when
     true.
3. **Resolve**: `mcp__humcase__update_note_status`
   `{"note_id": "<id>", "status": "resolved", "resolution_comment": "<report>"}`. A resolution
   without a report is refused.
4. **Sub-cases**: each task sub-case carries its own result. Resolve the ones that are done with
   their own report; name the others in the parent's Limits rather than resolving them silently.
5. **Cannot verify, or verification failed**: keep `in_progress`, post a progress report saying
   what blocks, and tell the user. An unverified result is not a result.

Example report (the user writes in English):

```markdown
**Done**: PDF export now scales images down to the page width before rendering
(`export/pdf.rs`); images over 4,000 px are resampled.

**Verified**: exported the 12-page case from the issue; all 9 images are present.
`cargo test export` passes (14 tests).

**Limits**: not tried with images over 20 MB; export time grows by about 1 s per 10 images.
```

## Handle a rework

A rework means the person reviewed the result and sent it back with a reason. It comes before
new work unless the user decides otherwise.

1. **Find the reworks**: `mcp__humcase__get_brief` (reworks pending, with the reason) when it
   exists, otherwise `mcp__humcase__get_handoff`, which lists them first under "Rework requested
   (do these first)".
2. **Read the reason**: `mcp__humcase__get_note`; `review.state` is `rework_requested` and
   `review.last_decision.comment` holds the reason, `rework_number` its rank. Compare with the
   previous report to see what the person rejected. When the reason is unclear, ask the user
   rather than guessing.
3. **Start again**: set the case to `in_progress`.
4. **Address the reason**, all of it, and only what the reason and the case ask for.
5. **Verify** as for any result.
6. **Resolve again** with a report that opens with what changed since the previous result and
   how it answers the reason ("Rework 2: ..."), then Done / Verified / Limits. Pass the previous
   report as `expected_resolution_comment` to replace it, or `"append": true` to keep it above.

## Results waiting for a decision

A case in `to_review` waits for the person. Do not modify it, re-resolve it or report on it to
hurry the decision. Tell the user which results wait for them, with their titles.

## After the result

- Unfinished work that keeps a date: `mcp__humcase__set_due_date`
  `{"note_id": "<id>", "due_at": "YYYY-MM-DD"}` (an empty string removes it), when the user
  agrees on the new day.
- New work found along the way (an issue, a follow-up task) becomes a sub-case of the case it
  comes from: see the `organize-cases` and `write-note` skills of this plugin.
