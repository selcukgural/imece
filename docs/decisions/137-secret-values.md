# What a secret's value is

The acceptance run of 2026-11-09 (real Claude Code sessions on a small
Python repository) showed a worker's diff line
`return sorted(self._books.values(), key=lambda b: b.title)` as
`return sorted(self._books.values(), [***] b: b.title)`. `i18n::clean`
hid `key=lambda`: its rule for `NAME=value` and `NAME:value` hid any word
whose name had `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `PASSWD` or
`CREDENTIAL` anywhere in it, whatever the value. On a Δ that is worse
than a secret shown: the person reviews code that is not what will be
applied, and `key=` is everywhere in Python (`sorted(key=…)`,
`dict(key=1)`, `cache_key=…`), as are `max_tokens=`, `password=getpass()`
and `api_key=os.environ[…]`.

The rule now asks two things, the name and the value.

The name is the run of ASCII letters, digits, `_` and `-` before the
sign (a closing quote left out: `"password":`), split into words at `_`,
`-` and a lower case letter or digit before an upper case one
(`name_words`: `apiKey` → `API` `KEY`). It names a secret when its last
word ends with `TOKEN`, `SECRET`, `PASSWORD`, `PASSWD`, `PASSPHRASE`,
`CREDENTIAL` or `CREDENTIALS` (`SECRET_WORDS`: `db_passwd`,
`accessToken`; not `TOKENS`, so `max_tokens` stays, and not
`token_count`), or is `KEY` after one of `KEY_KINDS` (`API`, `ACCESS`,
`SECRET`, `PRIVATE`, `SIGNING`, `CLIENT`, `AUTH`, `MASTER`, `ENCRYPTION`,
`LICENSE`), or ends with one of them glued to `KEY` (`APIKEY`,
`secretkey`). `KEY` alone is code's word. A name in capitals, the
environment's way (`SECRET_KEY_BASE`, `TOKEN_COUNT`), counts by any word,
deny by default.

The value is what follows the sign up to a `,` `;` or `&`, closing
brackets and sentence marks off its end, read from the word whole (the
brackets `core` takes off are what tell a call). An empty value after
`=` still hides (`API_KEY=`), after `:` it does not (`token: &str`). A
name in capitals hides any value. Any other name hides a value unless it
is code (`code_value`): a call, an index or a collection (`(` `[` `{`), a
dotted path of identifiers (`os.environ`, `self.token`), a shell variable
in capitals (`$DB_PASSWORD`, `${X}`, `$(…)`), one of `CODE_WORDS`
(`lambda`, `None`, `null`, `true` …), the name itself
(`password=password`) or an empty string literal. A quoted literal with
text is never code (`password="hunter2"`, `f"…"`, `rb'…'`), and a bare
word is hidden (`db_passwd=hunter2`, `token:abc`): between "looks like
code" and "looks like a secret" for a secret's name, only the shapes that
cannot hold a typed-in secret are let through.

Every `NAME=value` in a word is read, not only the first, so a call
without spaces is covered (`f(x=1,password="hunter2")`); `==` `!=` `<=`
`>=` `=>` `:=` and `::` are no assignment. A URL's parts keep the old
reading (a secret's name hides any value there: a query string holds no
code), with the new names (`?sort_key=name` now shows). The value is
also checked as a JSON Web Token and as an opaque run, as the whole word
was. The vendors' token starts, the word after `Bearer`/`Basic`, a URL's
password, JWTs and long opaque runs are as before; every case the old
tests required still holds.

Known gaps: a secret written as a separate word after `"api_key": ` (JSON
or YAML with a space) was never hidden and still is not; a password that
looks like a call or a dotted path (`p.w`) under a lower case name shows.
A test (`code_in_a_change_is_not_taken_for_a_secret`,
`a_secret_value_is_hidden_in_code_too`) lists both sides.

Read cost: `clean` runs on every drawn word; a word with no `=` or `:`
costs what it did. Failure: a name split at a byte that starts a
multi-byte letter panicked in the first version of this change (a name
is counted from its end in characters now, `→token=x`); a test holds it.
