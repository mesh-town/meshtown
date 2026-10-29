/* Node API types: the interface the exit traffic is bound to. Types only; the shared header and rules are in `api-types.ts`. */

/** `exit.interface` of the snapshot: the chosen interface and its state on this machine. */
export interface ExitIfaceInfo {
  name: string;
  state: "up" | "down" | "missing" | "unsupported";
}
