/**
 * # JobX 登录页
 *
 * 沿用控制台的「暖灰纸面 + 墨色文字 + 深青强调色」：左侧说明、右侧表单。
 * 登录逻辑统一走 `lib/auth-core.ts`。
 */

import { useState, type FormEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { AuthError, loginWithPassword } from '../../lib/auth-core';

/** 只接受应用内相对地址，避免 returnUrl 被用来跳外站 */
function safeReturnUrl(raw: string | null): string {
  if (!raw || !raw.startsWith('/') || raw.startsWith('//')) return '/';
  return raw;
}

export default function LoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const returnUrl = safeReturnUrl(searchParams.get('returnUrl'));
  const [account, setAccount] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    if (!account.trim() || !password) {
      setError('请输入账号与密码');
      return;
    }
    setLoading(true);
    try {
      await loginWithPassword(account, password);
      navigate(returnUrl, { replace: true });
    } catch (reason) {
      setError(reason instanceof AuthError ? reason.message : '登录失败，请稍后重试');
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-canvas px-4">
      <div className="w-full max-w-4xl overflow-hidden rounded-panel border border-line bg-surface shadow-panel">
        <div className="grid gap-0 md:grid-cols-[1.05fr_1fr]">
          <section className="flex flex-col justify-between gap-8 bg-accent-soft px-8 py-10">
            <div>
              <div className="inline-flex h-10 w-10 items-center justify-center rounded-xl bg-accent text-lg font-semibold text-white">
                J
              </div>
              <h1 className="mt-5 text-2xl font-semibold tracking-tight text-ink">JobX 控制台</h1>
              <p className="mt-2 max-w-sm text-sm leading-6 text-ink-soft">
                任务编排、分组与执行器管理。登录态在六个应用间共用，登出一次即全部失效。
              </p>
            </div>
            <p className="font-mono text-xs text-muted">
              JOBS · GROUPS · TASKS · EXECUTORS
            </p>
          </section>

          <section className="px-8 py-10">
            <h2 className="text-lg font-semibold text-ink">登录</h2>
            <p className="mt-1 text-sm text-muted">使用管理后台同一套账号密码</p>

            <form onSubmit={submit} className="mt-6 grid gap-4">
              <label className="grid gap-1.5 text-sm text-ink-soft">
                账号
                <input
                  autoFocus
                  value={account}
                  onChange={(event) => setAccount(event.target.value)}
                  placeholder="登录账号"
                  className="h-10 rounded-lg border border-line bg-sunken px-3 text-ink outline-none transition focus:border-accent focus:bg-surface"
                />
              </label>
              <label className="grid gap-1.5 text-sm text-ink-soft">
                密码
                <input
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder="登录密码"
                  className="h-10 rounded-lg border border-line bg-sunken px-3 text-ink outline-none transition focus:border-accent focus:bg-surface"
                />
              </label>
              {error && (
                <div className="rounded-lg border border-danger/30 bg-danger-soft px-3 py-2 text-sm text-danger">
                  {error}
                </div>
              )}
              <button
                type="submit"
                disabled={loading}
                className="mt-1 inline-flex h-10 items-center justify-center rounded-lg bg-accent px-4 text-sm font-medium text-white transition hover:bg-accent-strong disabled:cursor-not-allowed disabled:opacity-60"
              >
                {loading ? '登录中…' : '登录'}
              </button>
            </form>
          </section>
        </div>
      </div>
    </div>
  );
}
