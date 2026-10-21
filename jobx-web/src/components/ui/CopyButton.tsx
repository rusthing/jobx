import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { cn, copyText } from "@/lib/utils";

/** 复制按钮：ID / cron / JSON 到处都需要，点一下给个明确反馈 */
export function CopyButton({
  value,
  label = "复制",
  className,
}: {
  value: string;
  label?: string;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={async (event) => {
        event.stopPropagation();
        const ok = await copyText(value);
        if (!ok) return;
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1_400);
      }}
      className={cn(
        "inline-flex size-5 items-center justify-center rounded text-muted/70 transition-colors hover:bg-ink/[0.06] hover:text-ink",
        className,
      )}
    >
      {copied ? <Check className="size-3 text-success" /> : <Copy className="size-3" />}
    </button>
  );
}
