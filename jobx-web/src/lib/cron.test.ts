import { describe, expect, it } from "vitest";
import {
  CRON_PRESETS,
  CronError,
  describeCron,
  nextRuns,
  normalizeCron,
  parseCron,
  validateCron,
} from "./cron";

const at = (year: number, month: number, day: number, hour = 0, minute = 0, second = 0) =>
  new Date(year, month - 1, day, hour, minute, second).getTime();

describe("parseCron", () => {
  it("把 5 段补成 6 段（秒在前），与后端 cron crate 对齐", () => {
    const spec = parseCron("*/5 * * * *");
    expect(spec.padded).toBe(true);
    expect(spec.expression).toBe("0 */5 * * * *");
    expect(spec.seconds).toEqual([0]);
    expect(spec.minutes).toEqual([0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55]);
  });

  it("支持列表、区间、步长与英文缩写", () => {
    const spec = parseCron("0 30 9 * JAN-MAR MON-FRI");
    expect(spec.months).toEqual([1, 2, 3]);
    expect(spec.daysOfWeek).toEqual([1, 2, 3, 4, 5]);
    expect(spec.hours).toEqual([9]);
    expect(spec.minutes).toEqual([30]);
  });

  it("把 7 折成周日 0", () => {
    expect(parseCron("0 0 0 * * 7").daysOfWeek).toEqual([0]);
    expect(parseCron("0 0 0 * * SUN").daysOfWeek).toEqual([0]);
  });

  it("拒绝非法表达式", () => {
    expect(() => parseCron("")).toThrow(CronError);
    expect(() => parseCron("0 0 2 * *")).not.toThrow();
    expect(() => parseCron("0 0 25 * * *")).toThrow(/超出/);
    expect(() => parseCron("0 0 2 * * * 2027")).toThrow(/年份/);
    expect(() => parseCron("0 0 2 * * * *")).not.toThrow();
    expect(() => parseCron("0 0 2 L * *")).toThrow();
    expect(() => parseCron("0 0 2 * * *")).not.toThrow();
  });
});

describe("nextRuns", () => {
  it("每日 02:00 从当天 10:00 起算落在次日", () => {
    const runs = nextRuns("0 0 2 * * *", at(2026, 3, 10, 10, 0), 2);
    expect(runs[0]).toBe(at(2026, 3, 11, 2, 0));
    expect(runs[1]).toBe(at(2026, 3, 12, 2, 0));
  });

  it("当前分钟内的秒级触发点也会被算进来（只要时间还没到）", () => {
    const runs = nextRuns("30 * * * * *", at(2026, 3, 10, 10, 0, 10), 1);
    expect(runs[0]).toBe(at(2026, 3, 10, 10, 0, 30));
  });

  it("工作日 09:00 会跳过周末", () => {
    // 2026-03-13 是周五
    const runs = nextRuns("0 0 9 * * MON-FRI", at(2026, 3, 13, 12, 0), 1);
    expect(new Date(runs[0]).getDay()).toBe(1);
  });

  it("永不触发的组合返回空数组", () => {
    expect(nextRuns("0 0 0 30 2 *", at(2026, 1, 1), 1)).toEqual([]);
  });

  it("表达式合法但永不触发时 validateCron 给出提示", () => {
    expect(validateCron("0 0 0 30 2 *")).toMatch(/永远不会触发/);
    expect(validateCron("0 0 2 * * *")).toBeNull();
    expect(validateCron("0 0 2 * *")).toBeNull();
    expect(validateCron("nope")).not.toBeNull();
  });
});

describe("describeCron", () => {
  it("给出常见的口语化描述", () => {
    expect(describeCron("0 0 2 * * *")).toBe("每天 02:00");
    expect(describeCron("0 */5 * * * *")).toBe("每 5 分钟");
    expect(describeCron("0 0 9 * * MON-FRI")).toBe("每周一、周二、周三、周四、周五 09:00");
    expect(describeCron("0 30 9,18 * * *")).toBe("每天 09:30、18:30");
    expect(describeCron("0 0 0 1 * *")).toBe("每月 1 日 00:00");
  });

  it("不确定时返回 null，不瞎猜", () => {
    expect(describeCron("13 7 3 5 4 *")).toBeNull();
  });
});

describe("normalizeCron", () => {
  it("5 段补秒，6 段原样", () => {
    expect(normalizeCron("*/5 * * * *")).toBe("0 */5 * * * *");
    expect(normalizeCron("0 0 2 * * *")).toBe("0 0 2 * * *");
  });
});

describe("CRON_PRESETS", () => {
  it("每个模板都能被解析且有下一次触发时间", () => {
    for (const preset of CRON_PRESETS) {
      expect(validateCron(preset.expression), preset.label).toBeNull();
      expect(nextRuns(preset.expression, Date.now(), 1).length, preset.label).toBe(1);
    }
  });
});
