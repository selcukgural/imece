# Pinned projects

`:pin [NAME]` (2026-10-31, backlog C17 in #56, from cmux and Orca):
with several projects open, the one the person works in most kept its
place by the order they were opened in. Now it can stay at the top.

`:pin` pins the named project, else the one at hand; `:pin` again
unpins it (`Project::pinned`). The salon shows the open projects in
`World::salon_order`: the pinned ones first, then the rest, each group
in the order it was opened. When the salon is too short, calm workers
fold from the bottom of that order up, so a pinned project folds last.
A pinned project's header wears `▴` before its name (two of the name's
twelve columns), which the help's legend names. The flash says what
happened; the `:` menu shows the project `:pin` would act on, with `▴`
when it is pinned already; `:pin ` completes the open projects' names.
A closed project keeps its pin for when it is opened again in place.

The pin is the person's and outlives the run: `projects.pinned` in the
store (schema 7; a file of schema 6 gets the column, all unpinned), and
a pin alone is a change to save. The demo pins the same way, without a
store.

Not done (backlog C17's other half): a colour per project. The themes
name colours by role (ok, warn, info …) and their twelve speaker
colours are in a different order per theme, so a colour the person
chose by name would need a named palette in every theme first.
