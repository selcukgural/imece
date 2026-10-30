# Who leads

Who leads (2026-10-29, user's decision): no silent default. `--lead
VENDOR` wins; else the PRIMARY vendor from the preferences (`lead` in
the store's `meta`, see Leftovers v0) if it is on PATH and driveable; else the only driveable
vendor; else a question before the salon (`ui/pick.rs`: j/k, enter =
this time, `p` = make it the primary, esc = quit). `--lead ask` forces
the question; `:lead VENDOR` sets the primary from the next start on,
`:lead ask` forgets it. A primary that is not on PATH is said on the
question screen, never replaced silently. Decision is `choose_lead` in
`main.rs`, pure and tested.
