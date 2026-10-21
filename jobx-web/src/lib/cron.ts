/**
 * Cron 解析 / 预览 / 中文描述 —— 全部在浏览器侧完成。
 *
 * 后端用的是 Rust `cron` crate：**秒在前**（`sec min hour dom mon dow`），
 * 与 Linux crontab 的 5 段不同。运维同学习惯写 5 段，所以这里接受 5 段并自动补 `0` 秒；
 * 落库前再 `normalizeCron()` 统一成 6 段，避免服务端解析失败。
 *
 * 支持：`*` `?` `a` `a-b` `* /n` `a-b/n` `a/n`、逗号列表、月份/星期英文缩写。
 * 不支持 Quartz 的 `L` `W` `#`（后端也不支持，没必要造出前后端不一致的预览）。
 */

export class CronError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "CronError";
  }
}

export interface CronSpec {
  /** 归一化后的 6 段表达式 */
  expression: string;
  seconds: number[];
  minutes: number[];
  hours: number[];
  daysOfMonth: number[];
  months: number[];
  daysOfWeek: number[];
  /** 输入的是 5 段（已自动补秒） */
  padded: boolean;
}

const MONTH_NAMES: Record<string, number> = {
  JAN: 1, FEB: 2, MAR: 3, APR: 4, MAY: 5, JUN: 6,
  JUL: 7, AUG: 8, SEP: 9, OCT: 10, NOV: 11, DEC: 12,
};

const DOW_NAMES: Record<string, number> = {
  SUN: 0, MON: 1, TUE: 2, WED: 3, THU: 4, FRI: 5, SAT: 6,
};

const DOW_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function parseField(
  raw: string,
  min: number,
  max: number,
  label: string,
  names?: Record<string, number>,
): number[] {
  const values = new Set<number>();
  const toValue = (token: string): number => {
    const trimmed = token.trim();
    if (!trimmed) throw new CronError(`${label}字段为空`);
    const upper = trimmed.toUpperCase();
    if (names && upper in names) return names[upper];
    if (!/^\d+$/.test(trimmed)) {
      throw new CronError(`${label}字段「${trimmed}」无法识别`);
    }
    return Number(trimmed);
  };

  for (const part of raw.split(",")) {
    const body = part.trim();
    if (!body) throw new CronError(`${label}字段含空项`);

    let head = body;
    let step = 1;
    const slash = body.indexOf("/");
    if (slash >= 0) {
      head = body.slice(0, slash).trim();
      const stepText = body.slice(slash + 1).trim();
      if (!/^\d+$/.test(stepText) || Number(stepText) < 1) {
        throw new CronError(`${label}字段步长「${stepText}」非法`);
      }
      step = Number(stepText);
    }

    let lo: number;
    let hi: number;
    if (head === "*" || head === "?") {
      lo = min;
      hi = max;
    } else if (head.includes("-")) {
      const [a, b] = head.split("-");
      lo = toValue(a);
      hi = toValue(b);
    } else {
      lo = toValue(head);
      // `a/n` 表示从 a 起步到上界，`a` 单独出现表示仅 a
      hi = slash >= 0 ? max : lo;
    }

    if (lo < min || hi > max || lo > hi) {
      throw new CronError(`${label}字段「${body}」超出 ${min}-${max} 范围`);
    }

    for (let v = lo; v <= hi; v += step) {
      values.add(v);
    }
  }

  if (values.size === 0) throw new CronError(`${label}字段没有可选值`);
  return [...values].sort((a, b) => a - b);
}

/** 把 5 段表达式补成 6 段（秒在前），空串按调用方处理 */
export function normalizeCron(expression: string): string {
  const fields = expression.trim().split(/\s+/).filter(Boolean);
  if (fields.length === 5) return ["0", ...fields].join(" ");
  return fields.join(" ");
}

export function parseCron(expression: string): CronSpec {
  const raw = expression.trim().replace(/\s+/g, " ");
  if (!raw) throw new CronError("表达式不能为空");

  const fields = raw.split(" ");
  const count = fields.length;

  if (count === 7) {
    if (fields[6] !== "*") {
      throw new CronError("暂不支持年份字段（第 7 段），请留空或写 *");
    }
  } else if (count !== 5 && count !== 6) {
    throw new CronError(`需要 5 段或 6 段，当前 ${count} 段`);
  }

  const padded = count === 5;
  const [sec, min, hour, dom, mon, dow] = padded ? ["0", ...fields] : fields;

  // cron 里 7 也常被当作周日，统一折成 0
  const daysOfWeek = parseField(dow, 0, 7, "星期", DOW_NAMES)
    .map((day) => (day === 7 ? 0 : day))
    .filter((day, index, all) => all.indexOf(day) === index)
    .sort((a, b) => a - b);

  return {
    expression: (padded ? ["0", ...fields] : fields.slice(0, 6)).join(" "),
    seconds: parseField(sec, 0, 59, "秒"),
    minutes: parseField(min, 0, 59, "分"),
    hours: parseField(hour, 0, 23, "时"),
    daysOfMonth: parseField(dom, 1, 31, "日"),
    months: parseField(mon, 1, 12, "月", MONTH_NAMES),
    daysOfWeek,
    padded,
  };
}

export function isValidCron(expression: string): boolean {
  try {
    parseCron(expression);
    return true;
  } catch {
    return false;
  }
}

export function validateCron(expression: string): string | null {
  try {
    const spec = parseCron(expression);
    if (nextRunsFromSpec(spec, Date.now(), 1, 400).length === 0) {
      return "表达式永远不会触发，请检查日期与月份的组合";
    }
    return null;
  } catch (error) {
    return error instanceof CronError ? error.message : "表达式无法解析";
  }
}

function matchesDay(spec: CronSpec, date: Date): boolean {
  const month = date.getMonth() + 1;
  if (!spec.months.includes(month)) return false;
  // 后端（Rust cron）对「日」与「星期」同时限定时按“且”判定，这里保持一致
  if (!spec.daysOfMonth.includes(date.getDate())) return false;
  if (!spec.daysOfWeek.includes(date.getDay())) return false;
  return true;
}

/**
 * 求下一次触发时间。按天推进 + 命中的天里展开时/分/秒，最坏 400 天也只有几十万次
 * 廉价比较，比“逐秒试探”快几个数量级。
 */
export function nextRunsFromSpec(
  spec: CronSpec,
  fromMs: number,
  count = 5,
  horizonDays = 400,
): number[] {
  const results: number[] = [];
  const cursor = new Date(fromMs);

  for (let offset = 0; offset <= horizonDays && results.length < count; offset += 1) {
    const day = new Date(cursor.getFullYear(), cursor.getMonth(), cursor.getDate() + offset);
    if (!matchesDay(spec, day)) continue;

    for (const hour of spec.hours) {
      for (const minute of spec.minutes) {
        for (const second of spec.seconds) {
          const ts = new Date(
            day.getFullYear(),
            day.getMonth(),
            day.getDate(),
            hour,
            minute,
            second,
          ).getTime();
          if (ts > fromMs) {
            results.push(ts);
            if (results.length >= count) return results;
          }
        }
      }
    }
  }

  return results;
}

export function nextRuns(expression: string, fromMs = Date.now(), count = 5): number[] {
  return nextRunsFromSpec(parseCron(expression), fromMs, count);
}

function pad(value: number) {
  return String(value).padStart(2, "0");
}

function isFullRange(values: number[], min: number, max: number) {
  return values.length === max - min + 1 && values[0] === min;
}

function stepOf(values: number[]): number | null {
  if (values.length < 2) return null;
  const step = values[1] - values[0];
  if (step <= 0) return null;
  for (let i = 1; i < values.length; i += 1) {
    if (values[i] - values[i - 1] !== step) return null;
  }
  return step;
}

function joinNumbers(values: number[], unit: string) {
  if (values.length <= 3) return values.map((v) => `${v}${unit}`).join("、");
  return `${values.slice(0, 3).map((v) => `${v}${unit}`).join("、")} 等 ${values.length} 个`;
}

/**
 * 给一句人话。拿不准就返回 null，由 UI 展示原始表达式 —— 不猜，比猜错强。
 */
export function describeCron(expression: string): string | null {
  let spec: CronSpec;
  try {
    spec = parseCron(expression);
  } catch {
    return null;
  }

  const { seconds, minutes, hours, daysOfMonth, months, daysOfWeek } = spec;
  const everySecond = isFullRange(seconds, 0, 59);
  const everyMinute = isFullRange(minutes, 0, 59);
  const everyHour = isFullRange(hours, 0, 23);
  const everyDay = isFullRange(daysOfMonth, 1, 31);
  const everyMonth = isFullRange(months, 1, 12);
  const everyWeekday = isFullRange(daysOfWeek, 0, 6);

  if (!everyMonth) return null;

  const secondStep = stepOf(seconds);
  const minuteStep = stepOf(minutes);
  const hourStep = stepOf(hours);

  if (everySecond && everyMinute && everyHour && everyDay && everyWeekday) {
    return "每秒";
  }
  if (secondStep && seconds[0] === 0 && 60 % secondStep === 0 && everyMinute && everyHour && everyDay && everyWeekday) {
    return `每 ${secondStep} 秒`;
  }
  if (minutes.length === 1 && everyHour && everyDay && everyWeekday && seconds.length === 1) {
    if (minutes[0] === 0 && seconds[0] === 0) return "每小时整点";
    return seconds[0] === 0
      ? `每小时第 ${minutes[0]} 分`
      : `每小时 ${pad(minutes[0])}:${pad(seconds[0])}`;
  }
  if (minuteStep && minutes[0] === 0 && 60 % minuteStep === 0 && everyHour && everyDay && everyWeekday && seconds.length === 1) {
    return seconds[0] === 0
      ? `每 ${minuteStep} 分钟`
      : `每 ${minuteStep} 分钟（第 ${seconds[0]} 秒）`;
  }
  if (
    hourStep &&
    hours[0] === 0 &&
    24 % hourStep === 0 &&
    everyDay &&
    everyWeekday &&
    minutes.length === 1 &&
    seconds.length === 1
  ) {
    return `每 ${hourStep} 小时（第 ${minutes[0]} 分）`;
  }

  const timeOf = (hour: number, minute: number) =>
    seconds.length === 1 && seconds[0] === 0
      ? `${pad(hour)}:${pad(minute)}`
      : `${pad(hour)}:${pad(minute)}:${pad(seconds[0] ?? 0)}`;

  if (!everyDay && !everyWeekday) {
    // 日与星期同时限定时是「且」的关系，一句话说不清，交给原始表达式
    return null;
  }

  if (!everyDay || !everyWeekday) {
    const days = everyWeekday
      ? `每月 ${joinNumbers(daysOfMonth, " 日")}`
      : `每${[...daysOfWeek].sort((a, b) => a - b).map((day) => DOW_LABELS[day]).join("、")}`;

    if (everyHour && everyMinute) {
      return `${days} 全天（每分钟）`;
    }
    if (hours.length === 1 && minutes.length === 1) {
      return `${days} ${timeOf(hours[0], minutes[0])}`;
    }
    if (hours.length <= 4 && minutes.length === 1) {
      return `${days} ${hours.map((hour) => timeOf(hour, minutes[0])).join("、")}`;
    }
    return null;
  }

  if (hours.length === 1 && minutes.length === 1) {
    return `每天 ${timeOf(hours[0], minutes[0])}`;
  }
  if (hours.length <= 4 && minutes.length === 1) {
    return `每天 ${hours.map((hour) => timeOf(hour, minutes[0])).join("、")}`;
  }
  if (everyHour && minutes.length === 1) {
    return `每小时第 ${minutes[0]} 分`;
  }

  return null;
}

export interface CronPreset {
  label: string;
  expression: string;
}

/** 6 段（秒在前），直接可落库 */
export const CRON_PRESETS: CronPreset[] = [
  { label: "每 30 秒", expression: "*/30 * * * * *" },
  { label: "每分钟", expression: "0 * * * * *" },
  { label: "每 5 分钟", expression: "0 */5 * * * *" },
  { label: "每 10 分钟", expression: "0 */10 * * * *" },
  { label: "每小时整点", expression: "0 0 * * * *" },
  { label: "每天 02:00", expression: "0 0 2 * * *" },
  { label: "每天 09:30", expression: "0 30 9 * * *" },
  { label: "工作日 09:00", expression: "0 0 9 * * MON-FRI" },
  { label: "每周一 08:00", expression: "0 0 8 * * MON" },
  { label: "每月 1 日 00:30", expression: "0 30 0 1 * *" },
];
