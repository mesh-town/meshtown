/* The words of the "You in <network>" block on a network tab. `net` is the network name without quotes. */

export const youTitle = (net: string): string => `You in "${net}"`;
export const youNameLabel = (net: string): string => `Your name in "${net}"`;
export const YOU_NAME_PLACEHOLDER = "random name";
export const youNameHint = (net: string): string =>
  `Members of "${net}" see you by this name; leave it empty for a random one. Your other networks don’t see it.`;
export const youSameName = (first: string): string => `Use the same name as in "${first}"`;
export const youExitLabel = (net: string): string => `Let members of "${net}" open sites through this computer`;
export const youExitOff = (net: string): string => `Off: members of "${net}" can't open sites through this computer.`;
export const youLeaveHint = (net: string): string =>
  `Disconnect this computer from "${net}" for good. Your other networks stay. You'll need a new invite to come back.`;
export const YOU_LEAVE_BTN = "Leave network…";
