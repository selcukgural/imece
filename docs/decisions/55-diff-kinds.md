# Source, tests, generated

Backlog D6 in #57 (2026-10-31, from Orca), its first half: a change of
`+400 −20` might be twenty lines of code and a regenerated lock file,
and the person found out only by reading it.

`ui::diff::kind` sorts a changed file by its path alone: generated when
it is a lock file (`Cargo.lock`, `package-lock.json`, `yarn.lock`,
`pnpm-lock.yaml`, `go.sum`, `poetry.lock`, `Gemfile.lock`,
`composer.lock`), minified or generated code (`.min.js`, `.min.css`,
`.pb.go`, `_pb2.py`, `.g.dart`, `.Designer.cs`) or under a `generated/`,
`dist/` or `vendor/` directory; tests when under `tests/`, `test/`,
`__tests__/` or `spec/`, named `test_*` or ending in a test suffix
(`_test.go`, `_test.rs`, `_test.py`, `.test.ts`/`.js`/`.tsx`,
`.spec.ts`/`.js`, `Tests.cs`, `Test.java`); source otherwise. Generated
wins over tests (a fixture generated under `tests/` is generated).
Names only: nothing is read from the files.

When a change's files are of more than one kind, a line above its file
list (record 54) says each kind's lines and files, in that order:
`source +13 −3 (2) · tests +18 −0 (1) · generated +7 −5 (1)`, both
languages. A change of one kind shows no such line.

Not done here (D6b in #57): an editable commit-message suggestion from a
cheap tier. It is a paid model call, so it waited for the person's yes
on its ceilings; it came as `:commit` (record 56), asked for, not made
on every apply.
