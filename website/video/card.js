// Copied from kule@c09ec13545c331fd56d74ab76cb5f8557594a16c website/video/card.js (see VENDORED.md).
// Fills the card from its address: ?t=title&s=subtitle&l=last line.
"use strict";
const q = new URLSearchParams(location.search);
for (const [id, key] of [["title", "t"], ["sub", "s"], ["line", "l"]]) {
  if (q.has(key)) document.getElementById(id).textContent = q.get(key);
}
