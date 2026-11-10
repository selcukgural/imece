// tally: counts the words of a text.

export function words(text) {
  return text.split(/\s+/).filter((w) => w.length > 0);
}

export function tally(text) {
  const counts = new Map();
  for (const w of words(text)) {
    counts.set(w, (counts.get(w) ?? 0) + 1);
  }
  return counts;
}
