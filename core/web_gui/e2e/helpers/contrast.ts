// Contrast measurement of a page region (WCAG 2.x relative luminance), shared by the contrast spec.
// `measureRegion` runs in the page and returns raw colours; `checkRows` turns them into ratios and failures.
import type { Page } from '@playwright/test';

type Rgb = number[];

export interface RawRow {
  kind: 'text' | 'box';
  label: string;
  state: string;
  inactive: boolean;
  fg?: Rgb;
  bg?: Rgb;
  size?: string;
  weight?: string;
  border?: Rgb | null;
  fill?: Rgb | null;
  behind?: Rgb;
}

export interface Failure {
  where: string;
  theme: string;
  label: string;
  what: string;
  ratio: number;
  need: number;
}

// Runs inside the page: every visible text element and every control boundary below `rootSelector`.
function measureRegion(rootSelector: string): RawRow[] {
  const cv = document.createElement('canvas');
  cv.width = cv.height = 1;
  const cx = cv.getContext('2d', { willReadFrequently: true })!;
  const parse = (s: string): Rgb => {
    if (!s || s === 'transparent') return [0, 0, 0, 0];
    const m = s.match(/rgba?\(([^)]+)\)/);
    if (m) {
      const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
      return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1];
    }
    cx.clearRect(0, 0, 1, 1);
    cx.fillStyle = '#000';
    cx.fillStyle = s;
    cx.fillRect(0, 0, 1, 1);
    const d = cx.getImageData(0, 0, 1, 1).data;
    return [d[0], d[1], d[2], d[3] / 255];
  };
  const over = (top: Rgb, bot: Rgb): Rgb => {
    const a = top[3] + bot[3] * (1 - top[3]);
    if (!a) return [0, 0, 0, 0];
    return [0, 1, 2].map((i) => (top[i] * top[3] + bot[i] * bot[3] * (1 - top[3])) / a).concat(a);
  };
  const behind = (el: Element): Rgb => {
    const layers: Rgb[] = [];
    for (let n = el.parentElement; n; n = n.parentElement) {
      const c = parse(getComputedStyle(n).backgroundColor);
      if (c[3] > 0) layers.push(c);
      if (c[3] >= 1) break;
    }
    let acc: Rgb = layers.length && layers[layers.length - 1][3] >= 1 ? layers.pop()! : [255, 255, 255, 1];
    while (layers.length) acc = over(layers.pop()!, acc);
    return acc;
  };
  const opacityOf = (el: Element): number => {
    let o = 1;
    for (let n: Element | null = el; n; n = n.parentElement) o *= parseFloat(getComputedStyle(n).opacity);
    return o;
  };
  const visible = (el: Element): boolean => {
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none' && !el.closest('[hidden]');
  };
  const label = (el: Element): string => {
    const t = (el.getAttribute('aria-label') || el.textContent || el.getAttribute('placeholder') || '').trim().replace(/\s+/g, ' ').slice(0, 32);
    const cls = typeof el.className === 'string' && el.className ? '.' + el.className.trim().split(/\s+/).join('.') : '';
    return el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + cls + ' "' + t + '"';
  };
  const stateOf = (el: Element): string => {
    const b = el as HTMLButtonElement;
    const pressed = el.getAttribute('aria-pressed');
    return [b.disabled ? 'disabled' : '', pressed === 'true' ? 'pressed' : pressed ? 'unpressed' : '', el.getAttribute('aria-current') ? 'current' : ''].filter(Boolean).join(',');
  };
  // WCAG exempts disabled controls; the current step title is disabled by design and stays measured.
  const inactive = (el: Element): boolean => (el as HTMLButtonElement).disabled === true && !el.hasAttribute('aria-current');
  const withAlpha = (c: Rgb, op: number): Rgb => [c[0], c[1], c[2], c[3] * op];
  const root = [...document.querySelectorAll(rootSelector)].pop();
  if (!root) throw new Error('region not found: ' + rootSelector);
  const out: RawRow[] = [];
  for (const el of [root, ...root.querySelectorAll('*')]) {
    if (!visible(el)) continue;
    const cs = getComputedStyle(el);
    const bgBehind = behind(el);
    const op = opacityOf(el);
    const own = parse(cs.backgroundColor);
    const base = own[3] > 0 ? over(own, bgBehind) : bgBehind;
    const tag = el.tagName;
    const typed = ['INPUT', 'TEXTAREA', 'SELECT'].includes(tag);
    const hasText = [...el.childNodes].some((n) => n.nodeType === 3 && (n.textContent ?? '').trim());
    if (hasText || (typed && !['radio', 'checkbox', 'hidden'].includes((el as HTMLInputElement).type))) {
      out.push({ kind: 'text', label: label(el), state: stateOf(el), inactive: inactive(el), fg: over(withAlpha(parse(cs.color), op), base).slice(0, 3), bg: base.slice(0, 3), size: cs.fontSize, weight: cs.fontWeight });
    }
    const placeholder = (el as HTMLInputElement).placeholder;
    if (typed && placeholder) {
      const pc = parse(getComputedStyle(el, '::placeholder').color);
      out.push({ kind: 'text', label: label(el) + ' [placeholder]', state: '', inactive: inactive(el), fg: over(withAlpha(pc, op), base).slice(0, 3), bg: base.slice(0, 3), size: cs.fontSize, weight: cs.fontWeight });
    }
    const isCtl = (!el.matches('.home-tab') && ['BUTTON', 'INPUT', 'SELECT', 'TEXTAREA'].includes(tag)) || el.matches('.toggle-track, .step-num, [role=group]');
    const hiddenInput = tag === 'INPUT' && (cs.opacity === '0' || ['radio', 'checkbox'].includes((el as HTMLInputElement).type));
    if (isCtl && !hiddenInput) {
      const bw = parseFloat(cs.borderTopWidth) || 0;
      const bc = parse(cs.borderTopColor);
      const border = bw > 0 && bc[3] > 0 && cs.borderTopStyle !== 'none' ? over(withAlpha(bc, op), bgBehind).slice(0, 3) : null;
      const fill = own[3] > 0 ? over(withAlpha(own, op), bgBehind).slice(0, 3) : null;
      out.push({ kind: 'box', label: label(el), state: stateOf(el), inactive: inactive(el), border, fill, behind: bgBehind.slice(0, 3) });
    }
    if (el.matches('.toggle-track')) {
      // the unchecked look: the same track without a checked sibling
      const wrap = document.createElement('span');
      wrap.className = 'toggle';
      const clone = el.cloneNode(false) as HTMLElement;
      wrap.appendChild(clone);
      el.parentElement!.after(wrap);
      const f = parse(getComputedStyle(clone).backgroundColor);
      out.push({ kind: 'box', label: label(el) + ' [off look]', state: 'unchecked', inactive: false, border: null, fill: over(f, bgBehind).slice(0, 3), behind: bgBehind.slice(0, 3) });
      wrap.remove();
    }
  }
  return out;
}

const lum = (c: Rgb): number => {
  const f = (v: number): number => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
};
export const ratio = (a: Rgb, b: Rgb): number => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};
const hex = (c: Rgb): string => '#' + c.map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');

/** Measures the region and returns every element below the WCAG threshold (text 4.5 / large 3, boundaries 3). */
export async function contrastFailures(page: Page, rootSelector: string, where: string, theme: string): Promise<Failure[]> {
  const rows = await page.evaluate(measureRegion, rootSelector);
  const bad: Failure[] = [];
  for (const r of rows) {
    if (r.inactive) continue;
    if (r.kind === 'text') {
      const size = parseFloat(r.size ?? '14');
      const need = size >= 24 || (size >= 18.66 && Number(r.weight) >= 700) ? 3 : 4.5;
      const v = ratio(r.fg!, r.bg!);
      if (v < need) bad.push({ where, theme, label: r.label, what: `text ${hex(r.fg!)} on ${hex(r.bg!)}`, ratio: v, need });
      continue;
    }
    const vb = r.border ? ratio(r.border, r.behind!) : 0;
    const vf = r.fill ? ratio(r.fill, r.behind!) : 0;
    if (!r.border && !r.fill) continue;
    if (Math.max(vb, vf) < 3) bad.push({ where, theme, label: r.label + (r.state ? ' [' + r.state + ']' : ''), what: `boundary border ${vb.toFixed(2)} fill ${vf.toFixed(2)}`, ratio: Math.max(vb, vf), need: 3 });
  }
  return bad;
}

export const describeFailure = (f: Failure): string => `${f.theme} ${f.where}: ${f.label} — ${f.what} = ${f.ratio.toFixed(2)} (need ${f.need})`;
