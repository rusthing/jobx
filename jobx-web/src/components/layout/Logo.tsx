export function LogoMark({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden>
      <rect width="32" height="32" rx="8" fill="#191b16" />
      <circle cx="16" cy="16" r="8.6" fill="none" stroke="#6fd0ae" strokeWidth="2.1" />
      <path
        d="M16 10.8v5.6l3.6 2.1"
        fill="none"
        stroke="#6fd0ae"
        strokeWidth="2.1"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx="16" cy="16" r="1.2" fill="#6fd0ae" />
    </svg>
  );
}

export function Wordmark() {
  return (
    <div className="flex items-center gap-2.5">
      <LogoMark />
      <div className="leading-none">
        <div className="text-[14.5px] font-semibold tracking-[-0.01em] text-ink">JobX</div>
        <div className="mt-[3px] text-[10.5px] font-medium tracking-[0.08em] text-muted">
          调度控制台
        </div>
      </div>
    </div>
  );
}
