# Researchers

Researchers (an optional layer on top, not built, no longer a
precondition): a capped pool of short-lived researchers answers
questions for coders that are told not to read the outside world
themselves; identical questions in flight are merged and answers cached.
Graded taint: reading a report makes a coder soft-tainted (only
sensitive changes need the person); researchers are fully tainted.
Trusted sources: official docs by default; user-content sites never;
package-registry docs only for packages in the lockfile.
