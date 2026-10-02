/** Keep the visible prefix when a partial hypothesis revises its suffix. */
export function captionUpdate(previous: string[], displayed: string[], next: string[]) {
  let common = 0;
  while (common < previous.length && common < next.length && previous[common] === next[common]) {
    common++;
  }
  const displayStart = Math.max(0, previous.length - displayed.length);
  const start = common < displayStart ? 0 : displayStart;
  return { words: next.slice(start), keep: Math.max(0, common - start) };
}
