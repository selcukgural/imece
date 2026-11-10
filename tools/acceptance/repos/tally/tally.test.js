import { test } from "node:test";
import assert from "node:assert/strict";
import { tally, words } from "./tally.js";

test("words splits on any space", () => {
  assert.deepEqual(words(" a  b\nc "), ["a", "b", "c"]);
});

test("tally counts each word", () => {
  assert.equal(tally("a b a").get("a"), 2);
});
