/** Window-control dots: mint leader, two hollow rings. Decorative. */
export default function WindowDots() {
  return (
    <span className="inline-flex items-center gap-1.5" aria-hidden="true">
      <span className="size-2.5 rounded-full bg-primary" />
      <span className="size-2.5 rounded-full border border-border bg-transparent" />
      <span className="size-2.5 rounded-full border border-border bg-transparent" />
    </span>
  );
}