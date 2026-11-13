/* Texts for a chosen exit that is overloaded: the Home panel, the header and the ring legend. */

export const busyMainText = (who: string): string => `Sites open via ${who}, but it is overloaded`;
export const busyNetMainText = (who: string, net: string): string => `Sites open via ${who} (${net}), but it is overloaded`;
export const busyHeaderText = (who: string): string => `Sites open via ${who} — overloaded`;
export const busyHeaderNetText = (who: string, net: string): string => `Sites open via ${who} (${net}) — overloaded`;

export const BUSY_MAY_NOT_LOAD = "Some sites and apps may not load.";
export const busyTurnedAwayText = (who: string, refused: number, attempts: number): string =>
  `${who} turned away ${refused} of your last ${attempts} connections.`;

export const busyAdviceText = (who: string): string =>
  `Choose another member to open sites through, or ask the owner of ${who} to raise its connection limit.`;
export const busyAdviceMemoryText = (who: string): string =>
  `Choose another member to open sites through, or ask the owner of ${who} to free up memory on it.`;

export const busyLegendText = (label: string, relayed = false): string =>
  `${label} — your exit, overloaded${relayed ? " — through another member" : ""}`;
export const HOME_RING_SINGLE_VIA_BUSY = "your exit, overloaded";
