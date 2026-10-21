import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { useBodyScrollLock } from "@/hooks/use-body-scroll-lock";
import { cn } from "@/lib/utils";
import { IconButton } from "./Button";

export interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  subtitle?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
  /** 标题区的额外内容（如状态徽标） */
  badge?: ReactNode;
}

export function Drawer({
  open,
  onClose,
  title,
  subtitle,
  children,
  footer,
  width = 580,
  badge,
}: DrawerProps) {
  useBodyScrollLock(open);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50 flex justify-end">
      {/*
        遮罩**不要**加 `backdrop-blur`：一层覆盖整个视口的 backdrop-filter 会让浏览器每帧
        重新模糊遮罩下的整页（实测抽屉内滚动 55fps → 30fps，Retina 上按物理像素再翻 4 倍）。
        纯半透明色同样能拉开层次，成本为零。
      */}
      <div className="absolute inset-0 animate-fade bg-ink/25" onClick={onClose} aria-hidden />
      <div
        role="dialog"
        aria-modal="true"
        className="relative flex h-full max-w-full animate-slide-left flex-col border-l border-line bg-surface shadow-float"
        style={{ width }}
      >
        <header className="flex items-start justify-between gap-3 border-b border-line px-5 py-3.5">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h2 className="truncate text-[15px] font-semibold text-ink">{title}</h2>
              {badge}
            </div>
            {subtitle && <div className="mt-1 text-[12px] text-muted">{subtitle}</div>}
          </div>
          <IconButton label="关闭" onClick={onClose}>
            <X className="size-4" />
          </IconButton>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>

        {footer && (
          <footer className="border-t border-line bg-sunken/70 px-5 py-3">{footer}</footer>
        )}
      </div>
    </div>,
    document.body,
  );
}

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description?: ReactNode;
  confirmText?: string;
  tone?: "danger" | "primary";
  loading?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmText = "确认",
  tone = "danger",
  loading,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  useBodyScrollLock(open);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onCancel]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-[60] flex items-center justify-center p-6">
      <div className="absolute inset-0 animate-fade bg-ink/30" onClick={onCancel} aria-hidden />
      <div
        role="alertdialog"
        aria-modal="true"
        className="relative w-full max-w-[400px] animate-rise rounded-[14px] border border-line bg-surface p-5 shadow-float"
      >
        <h2 className="text-[14.5px] font-semibold text-ink">{title}</h2>
        {description && (
          <div className="mt-1.5 text-[12.5px] leading-relaxed text-ink-soft">{description}</div>
        )}
        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="h-8 rounded-lg border border-line px-3 text-[13px] font-medium text-ink-soft hover:bg-sunken"
          >
            取消
          </button>
          <button
            type="button"
            disabled={loading}
            onClick={onConfirm}
            className={cn(
              "h-8 rounded-lg px-3 text-[13px] font-medium text-white disabled:opacity-50",
              tone === "danger" ? "bg-danger hover:brightness-95" : "bg-accent hover:bg-accent-strong",
            )}
          >
            {loading ? "处理中…" : confirmText}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
