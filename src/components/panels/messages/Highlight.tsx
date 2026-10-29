/** Marks the first case-insensitive occurrence of `query` in `text`. */
export function Highlight({ text, query }: { text: string; query: string }) {
  const q = query.trim();
  const at = q ? text.toLowerCase().indexOf(q.toLowerCase()) : -1;
  if (at < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, at)}
      <mark className="rounded-[3px] bg-accent/25 px-px text-inherit">{text.slice(at, at + q.length)}</mark>
      {text.slice(at + q.length)}
    </>
  );
}
