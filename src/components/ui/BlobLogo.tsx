export function BlobLogo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" className={className} aria-hidden>
      <defs>
        <linearGradient id="blob-g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="rgb(var(--c-accent))" />
          <stop offset="1" stopColor="rgb(var(--c-accent2))" />
        </linearGradient>
      </defs>
      <path
        d="M33 6c10 0 21 6 24 17s-2 23-11 29-22 7-30 0S3 35 6 24 22 6 33 6z"
        fill="url(#blob-g)"
      />
      <ellipse cx="25" cy="31" rx="3.2" ry="4.6" fill="#fff" />
      <ellipse cx="38" cy="31" rx="3.2" ry="4.6" fill="#fff" />
      <ellipse cx="20" cy="20" rx="7" ry="4" fill="#fff" opacity=".25" />
    </svg>
  );
}
