/** Android's own Back / Home / Recents glyphs. Shared by the main and stream windows. */

export const BackGlyph = () => (
  <svg viewBox="0 0 24 24" className="size-[18px]" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinejoin="round" aria-hidden="true">
    <path d="M16.5 5.5v13L6.5 12z" />
  </svg>
);

export const HomeGlyph = () => (
  <svg viewBox="0 0 24 24" className="size-[18px]" fill="none" stroke="currentColor" strokeWidth="1.9" aria-hidden="true">
    <circle cx="12" cy="12" r="6.5" />
  </svg>
);

export const RecentGlyph = () => (
  <svg viewBox="0 0 24 24" className="size-[18px]" fill="none" stroke="currentColor" strokeWidth="1.9" aria-hidden="true">
    <rect x="6" y="6" width="12" height="12" rx="2.5" />
  </svg>
);
