import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { CircleSlash, FolderOpen, Pencil, Plus, RefreshCw, Search, Trash2 } from "lucide-react";
import { PageHeader } from "@/components/page";
import { Panel, PanelState, Skeleton } from "@/components/ui/Panel";
import { Button, IconButton } from "@/components/ui/Button";
import { Input, Textarea, Field } from "@/components/ui/Field";
import { ConfirmDialog, Drawer } from "@/components/ui/Drawer";
import { useToast } from "@/components/ui/Toast";
import { useAllGroups, useGroupMutations } from "@/hooks/use-groups";
import { useAllJobs } from "@/hooks/use-jobs";
import { setDataMode, type Group } from "@/lib/api";
import { formatCompact, formatRelative } from "@/lib/time";
import { useNow } from "@/hooks/use-now";

type Editor = { mode: "new" } | { mode: "edit"; group: Group } | null;

interface FormState {
  name: string;
  remark: string;
}

const EMPTY_FORM: FormState = { name: "", remark: "" };

function formOf(group: Group): FormState {
  return { name: group.name, remark: group.remark ?? "" };
}

export function GroupsPage() {
  const toast = useToast();
  const now = useNow(60_000);
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const [keywordInput, setKeywordInput] = useState("");
  const [editor, setEditor] = useState<Editor>(null);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [deleting, setDeleting] = useState<Group | null>(null);

  const { data, isLoading, isFetching, error, refetch } = useAllGroups();
  const allJobs = useAllJobs();
  const { create, update, remove } = useGroupMutations();

  // 深链：/groups?new=1 直接打开新建
  const urlNew = searchParams.get("new");
  useEffect(() => {
    if (urlNew === "1") {
      setEditor({ mode: "new" });
      setForm(EMPTY_FORM);
      setSearchParams({}, { replace: true });
    }
  }, [urlNew, setSearchParams]);

  /* 分组数量少，搜索直接在已拉全量的客户端做，不值得为此走后端分页 */
  const groups = useMemo(() => {
    const needle = keywordInput.trim().toLowerCase();
    const list = [...(data ?? [])].sort((a, b) => a.name.localeCompare(b.name, "zh-Hans-CN"));
    if (!needle) return list;
    return list.filter(
      (group) =>
        group.name.toLowerCase().includes(needle) ||
        (group.remark ?? "").toLowerCase().includes(needle),
    );
  }, [data, keywordInput]);

  /*
   * 计划数不冗余到分组上，浏览器侧按全量计划聚合：分组改名数字照旧准确，
   * 代价是和计划列表一样吃 500 条的量级上限 —— 分组是人工维护的，够用。
   */
  const jobCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const job of allJobs.data ?? []) {
      if (job.groupId) counts.set(job.groupId, (counts.get(job.groupId) ?? 0) + 1);
    }
    return counts;
  }, [allJobs.data]);

  const openNew = () => {
    setForm(EMPTY_FORM);
    setEditor({ mode: "new" });
  };

  const openEdit = (group: Group) => {
    setForm(formOf(group));
    setEditor({ mode: "edit", group });
  };

  const submit = () => {
    const input = {
      name: form.name.trim(),
      remark: form.remark.trim() ? form.remark.trim() : null,
    };
    if (!input.name) {
      toast.error("名称不能为空", "分组名 1–50 个字符");
      return;
    }
    const isEdit = editor?.mode === "edit";
    const mutation = isEdit ? update : create;
    mutation.mutate(isEdit ? { id: (editor as { group: Group }).group.id, ...input } : input, {
      onSuccess: (saved) => {
        toast.success(isEdit ? "分组已更新" : "分组已创建", `${saved.name} · #${saved.id}`);
        setEditor(null);
      },
      onError: (mutationError) => toast.error("保存失败", mutationError.message),
    });
  };

  const confirmDelete = () => {
    if (!deleting) return;
    remove.mutate(deleting.id, {
      onSuccess: () => {
        toast.success("分组已删除", deleting.name);
        setDeleting(null);
      },
      onError: (mutationError) => {
        toast.error("删除失败", mutationError.message);
        setDeleting(null);
      },
    });
  };

  const total = data?.length ?? 0;

  return (
    <>
      <PageHeader
        title="计划分组"
        subtitle={
          <>
            只是归类维度，不参与调度计算 · 共{" "}
            <span className="tnum font-mono text-ink-soft">{total}</span> 个
          </>
        }
        actions={
          <>
            <Button
              size="sm"
              variant="ghost"
              icon={<RefreshCw className={isFetching ? "animate-spin" : undefined} />}
              onClick={() => refetch()}
            >
              刷新
            </Button>
            <Button size="sm" variant="primary" icon={<Plus className="size-3.5" />} onClick={openNew}>
              新建分组
            </Button>
          </>
        }
      />

      <Panel className="overflow-hidden">
        <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2.5">
          <div className="relative min-w-[200px] flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted" />
            <Input
              value={keywordInput}
              onChange={(event) => setKeywordInput(event.target.value)}
              placeholder="搜索分组名、备注…"
              className="h-8 pl-8 text-[12.5px]"
            />
          </div>
          <span className="ml-auto text-[11px] text-muted">
            共 <span className="tnum font-mono text-ink-soft">{groups.length}</span> 个
            {keywordInput.trim() ? "（已过滤）" : ""}
          </span>
        </div>

        {error ? (
          <PanelState
            icon={<CircleSlash className="size-4" />}
            title="分组列表加载失败"
            description={error.message}
            action={
              <div className="flex gap-2">
                <Button size="sm" onClick={() => refetch()}>
                  重试
                </Button>
                <Button size="sm" variant="primary" onClick={() => setDataMode("mock")}>
                  切换到演示数据
                </Button>
              </div>
            }
          />
        ) : isLoading ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 4 }).map((_, index) => (
              <Skeleton key={index} className="h-12" />
            ))}
          </div>
        ) : groups.length === 0 ? (
          <PanelState
            icon={<Search className="size-4" />}
            title={keywordInput.trim() ? "没有符合条件的分组" : "还没有分组"}
            description={
              keywordInput.trim()
                ? "换个关键词再试。"
                : "建几个分组（如「订单与库存」「报表与账单」），再在计划里归组，列表即可按组筛选。"
            }
            action={
              keywordInput.trim() ? (
                <Button size="sm" onClick={() => setKeywordInput("")}>
                  清空搜索
                </Button>
              ) : (
                <Button size="sm" variant="primary" icon={<Plus className="size-3.5" />} onClick={openNew}>
                  新建分组
                </Button>
              )
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[720px] border-collapse text-left">
              <thead>
                <tr className="border-b border-line bg-sunken/60 text-[11px] text-muted">
                  <th className="px-4 py-2 font-medium">分组</th>
                  <th className="px-3 py-2 font-medium">备注</th>
                  <th className="px-3 py-2 font-medium">计划数</th>
                  <th className="px-3 py-2 font-medium">更新时间</th>
                  <th className="px-4 py-2 text-right font-medium">操作</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line/70">
                {groups.map((group) => {
                  const count = jobCounts.get(group.id) ?? 0;
                  return (
                    <tr key={group.id} className="group transition-colors hover:bg-sunken/50">
                      <td className="max-w-[260px] px-4 py-2.5">
                        <button
                          type="button"
                          onClick={() => openEdit(group)}
                          className="flex items-center gap-1.5 text-[13px] font-medium text-ink hover:text-accent"
                        >
                          <FolderOpen className="size-3.5 shrink-0 text-accent/70" />
                          <span className="truncate">{group.name}</span>
                        </button>
                        <div className="mt-0.5 font-mono text-[10px] text-muted/80">#{group.id}</div>
                      </td>
                      <td className="max-w-[280px] px-3 py-2.5">
                        {group.remark ? (
                          <span className="line-clamp-2 text-[12px] text-ink-soft">{group.remark}</span>
                        ) : (
                          <span className="text-[11.5px] text-muted/70">—</span>
                        )}
                      </td>
                      <td className="px-3 py-2.5">
                        {count > 0 ? (
                          <button
                            type="button"
                            onClick={() => navigate(`/jobs?group=${group.id}`)}
                            className="tnum rounded-md px-1.5 py-0.5 font-mono text-[12px] text-accent-strong hover:bg-accent-soft"
                            title="在计划列表中查看该分组"
                          >
                            {count}
                          </button>
                        ) : (
                          <span className="tnum font-mono text-[12px] text-muted/70">0</span>
                        )}
                      </td>
                      <td className="px-3 py-2.5">
                        <div className="tnum font-mono text-[12px] text-ink">
                          {formatRelative(group.updateMs, now)}
                        </div>
                        <div className="text-[10px] text-muted">{formatCompact(group.updateMs)}</div>
                      </td>
                      <td className="px-4 py-2.5">
                        <div className="flex items-center justify-end gap-0.5 opacity-70 transition-opacity group-hover:opacity-100">
                          <IconButton label="编辑" onClick={() => openEdit(group)}>
                            <Pencil className="size-3.5" />
                          </IconButton>
                          <IconButton
                            label="删除"
                            tone="danger"
                            onClick={() => setDeleting(group)}
                          >
                            <Trash2 className="size-3.5" />
                          </IconButton>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Panel>

      <p className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-[10.5px] leading-relaxed text-muted/80">
        删除受外键保护：分组下仍有计划时服务端会拒绝（<code className="font-mono text-[11px]">on delete restrict</code>），
        先把计划移到别的组或归为未分组。
      </p>

      <Drawer
        open={editor !== null}
        onClose={() => setEditor(null)}
        title={editor?.mode === "edit" ? "编辑分组" : "新建分组"}
        subtitle="分组只承担归类，改名字不影响计划与调度"
        width={420}
        footer={
          <div className="flex justify-end gap-2">
            <Button size="sm" onClick={() => setEditor(null)}>
              取消
            </Button>
            <Button
              size="sm"
              variant="primary"
              loading={create.isPending || update.isPending}
              onClick={submit}
            >
              保存
            </Button>
          </div>
        }
      >
        <div className="space-y-4">
          <Field label="名称" required hint="1–50 个字符，同库内唯一">
            <Input
              value={form.name}
              onChange={(event) => setForm((prev) => ({ ...prev, name: event.target.value }))}
              placeholder="例：订单与库存"
              maxLength={50}
              autoFocus
            />
          </Field>
          <Field label="备注" hint="给人看的说明，可为空">
            <Textarea
              value={form.remark}
              onChange={(event) => setForm((prev) => ({ ...prev, remark: event.target.value }))}
              placeholder="例：电商域的同步与对账类计划"
              rows={3}
              maxLength={200}
            />
          </Field>
        </div>
      </Drawer>

      <ConfirmDialog
        open={deleting !== null}
        title={`删除分组「${deleting?.name ?? ""}」？`}
        description={
          <>
            服务端外键为 <code className="font-mono text-[11px]">on delete restrict</code>，
            若该分组下仍有计划，删除会被拒绝。
          </>
        }
        confirmText="删除"
        loading={remove.isPending}
        onConfirm={confirmDelete}
        onCancel={() => setDeleting(null)}
      />
    </>
  );
}
