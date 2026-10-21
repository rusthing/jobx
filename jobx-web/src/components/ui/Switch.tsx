import { cn } from "@/lib/utils";

export interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  label?: string;
  size?: "sm" | "md";
}

export function Switch({ checked, onChange, disabled, label, size = "md" }: SwitchProps) {
  const track = size === "sm" ? "h-[18px] w-[32px]" : "h-5 w-9";
  const knob = size === "sm" ? "size-3.5" : "size-4";
  const shift = size === "sm" ? "translate-x-[14px]" : "translate-x-4";

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex shrink-0 items-center rounded-full border transition-colors duration-200",
        track,
        checked
          ? "border-accent-strong/30 bg-accent"
          : "border-line-strong/60 bg-[#e6e8e1] hover:bg-[#dde0d8]",
        disabled && "cursor-not-allowed opacity-50",
      )}
    >
      <span
        className={cn(
          "ml-[2px] rounded-full bg-white shadow-[0_1px_2px_rgb(25_27_22/0.25)] transition-transform duration-200",
          knob,
          checked ? shift : "translate-x-0",
        )}
      />
    </button>
  );
}

export interface SegmentedOption<T extends string | number> {
  value: T;
  label: string;
  count?: number;
}

export function Segmented<T extends string | number>({
  options,
  value,
  onChange,
  size = "md",
  className,
}: {
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  size?: "sm" | "md";
  className?: string;
}) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-0.5 rounded-lg border border-line bg-sunken p-0.5",
        className,
      )}
    >
      {options.map((option) => {
        const active = option.value === value;
        return (
          <button
            key={String(option.value)}
            type="button"
            onClick={() => onChange(option.value)}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-[6px] font-medium transition-colors",
              size === "sm" ? "h-6 px-2 text-[12px]" : "h-7 px-2.5 text-[12.5px]",
              active
                ? "bg-surface text-ink shadow-[0_1px_2px_rgb(25_27_22/0.08)]"
                : "text-muted hover:text-ink-soft",
            )}
          >
            {option.label}
            {option.count !== undefined && (
              <span
                className={cn(
                  "tnum text-[10.5px]",
                  active ? "text-ink-soft" : "text-muted/80",
                )}
              >
                {option.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}
