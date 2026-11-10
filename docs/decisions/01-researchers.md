# Researchers

**Closed, not to be built (2026-11-10).** Record 26 (posture change,
2026-10-30) gave workers and helpers the web and the shell themselves, so
a separate layer of researchers answering coders who may not read the
outside world guards nothing. On Lonca the taint is the store's own
(records 119-121), so graded taint has no place to live either. The code
still has a researcher cap and rows (grep `researcher` under `crates/`);
removing them is a follow-up, not done with this record.

The text below is the proposal as it was written, kept as history.

Researchers (an optional layer on top, not built, no longer a
precondition): a capped pool of short-lived researchers answers
questions for coders that are told not to read the outside world
themselves; identical questions in flight are merged and answers cached.
Graded taint: reading a report makes a coder soft-tainted (only
sensitive changes need the person); researchers are fully tainted.
Trusted sources: official docs by default; user-content sites never;
package-registry docs only for packages in the lockfile.
