import { expect, test, type Locator, type Page } from '@playwright/test';

import { GLOSSARY } from '../glossary';

// The first radio button is "Directly, without an exit"; the rest are exit candidates.
const CANDIDATE_NAME = /^(?!Directly)/;

/** Boxes of the radio button and the label of one exit list row. */
export interface RadioRow {
  inputLeft: number;
  inputRight: number;
  labelLeft: number;
}

/**
 * The "Exit via…" card on the first screen: the exit list (radio buttons), the hint,
 * the route status line. Merges `ExitSection`, `HomeExit` and part of `HomeUx`.
 */
export class HomeExitCard {
  /** The exit list. Three names of one locator — from three old classes (`root`, `list`, `exitList`). */
  readonly root: Locator;
  readonly list: Locator;
  /** The radio button "Directly, without an exit". */
  readonly direct: Locator;
  /** Radio buttons of exit candidates (`choices` from `ExitSection`, `exitBoxes` from `HomeUx`). */
  readonly choices: Locator;
  readonly exitBoxes: Locator;
  readonly emptyHint: Locator;
  readonly exitHint: Locator;
  /** The whole card box — for measuring the bottom. */
  readonly card: Locator;
  /** List rows (label wrappers of the radio buttons). */
  readonly exitRows: Locator;
  readonly candidateRows: Locator;

  constructor(page: Page) {
    // anchor — id: the list container has no role; the card next to it has the "Check a site" field, and the radio buttons are only here
    this.root = page.locator('#exit-list');
    this.list = this.root;
    this.direct = this.root.getByRole('radio', { name: GLOSSARY.directChoice });
    this.choices = this.root.getByRole('radio', { name: CANDIDATE_NAME });
    this.exitBoxes = this.choices;
    this.emptyHint = this.root.getByText('No other member offers');
    this.exitHint = page.getByText(/^Sites go through the one member you choose/);
    this.card = page.getByRole('region', { name: 'Exit via…' });
    // a list row is a `label` without a role; the box of the whole row is needed
    this.exitRows = this.root.locator('label');
    this.candidateRows = this.exitRows.filter({ hasNotText: GLOSSARY.directChoice });
  }

  /** The candidate's radio button by label (the name and `(id)`, or the short id of an unnamed one). */
  choice(label: string): Locator {
    return this.root.getByRole('radio', { name: label });
  }

  /** The exit choice radio button by the member's `id_full` (was `HomeExit.choice(idFull)`). */
  choiceById(idFull: string): Locator {
    // anchor — `data-id`: the row label is the member's name, `id_full` is only there
    return this.root.locator(`input[data-id="${idFull}"]`);
  }

  /** A click on the radio button — the row's label wrapper accepts the press. */
  async choose(radio: Locator): Promise<void> {
    await radio.click();
  }

  /** Choosing "Directly, without an exit": the exit is off, `route.exits == []`. */
  async chooseDirect(): Promise<void> {
    await this.choose(this.direct);
  }

  /** Choosing an exit candidate by label (see `choice`). */
  async choosePeer(label: string): Promise<void> {
    await this.choose(this.choice(label));
  }

  /** Returns the tops of all exit list rows ("as usual" and candidates), top to bottom. */
  async rowTops(): Promise<number[]> {
    const tops: number[] = [];
    for (const row of await this.exitRows.all()) {
      const box = await row.boundingBox();
      if (box) {
        tops.push(box.y);
      }
    }
    return tops;
  }

  /** How many candidate rows are checked. */
  async checkedCount(): Promise<number> {
    let n = 0;
    for (const box of await this.exitBoxes.all()) {
      if (await box.isChecked()) {
        n += 1;
      }
    }
    return n;
  }

  /** Boxes of the radio button and the label of each list row. */
  async radioRows(): Promise<RadioRow[]> {
    const out: RadioRow[] = [];
    for (const row of await this.exitRows.all()) {
      const input = await row.getByRole('radio').boundingBox();
      // getByText takes the innermost element with the text — the row label.
      const label = await row.getByText(/\S/).first().boundingBox();
      out.push({
        inputLeft: input?.x ?? NaN,
        inputRight: input ? input.x + input.width : NaN,
        labelLeft: label?.x ?? NaN,
      });
    }
    return out;
  }

  /** All rows are radio buttons, left edges on one vertical, the radio button to the left of the label (no rows — a skip annotation). */
  async expectRadiosAligned(): Promise<void> {
    const rows = await this.radioRows();
    if (rows.length === 0) {
      test.info().annotations.push({ type: 'skip', description: 'not checked: the exit list has no rows on this target' });
      return;
    }
    await expect.soft(this.root.getByRole('textbox'), 'not radio buttons: input fields').toHaveCount(0);
    await expect.soft(this.root.getByRole('checkbox'), 'not radio buttons: checkboxes').toHaveCount(0);
    const edges = rows.map((r) => r.inputLeft);
    expect.soft(Math.max(...edges) - Math.min(...edges), `left edges=${edges}`).toBeLessThanOrEqual(1);
    for (const r of rows) {
      expect.soft(r.inputRight, `radio button is to the right of the label: ${JSON.stringify(r)}`).toBeLessThanOrEqual(r.labelLeft);
    }
  }
}
