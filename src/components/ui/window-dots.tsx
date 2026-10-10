/** Neobrutalist window-control dots: black outlines, lime leader. Decorative. */
export default function WindowDots() {
  return (
    <span className="inline-flex items-center gap-1.5" aria-hidden="true">
      <span className="size-2.5 rounded-full border-2 border-ink bg-lime" />
      <span className="size-2.5 rounded-full border-2 border-ink bg-white" />
      <span className="size-2.5 rounded-full border-2 border-ink bg-white" />
    </span>
  );
}
