/** The network name in status texts (the page's `NET_NAME`). */
export const NET_NAME = 'Mesh Town';

/** Words of the program's single dictionary: visible texts are kept in one place. */
export const GLOSSARY = {
  /** Heading of the first column of the domain table. */
  siteColumn: 'Site',
  /** Heading of the "For you" column in the domain table. */
  youColumn: 'For you',
  /** Label of the first radio button of the exit list (exit is off). */
  directChoice: `Directly, without an exit`,
  /** Button of a member row: use this member as the exit (for an exit chosen by name — `: <name>`). Same idea as the tray submenu "Exit via {name}", but where the name is already shown, the button says "Use as exit". */
  exitViaButton: 'Use as exit',
  /** One phrase explaining the word "exit": shown in the exit choice card and on step 3 of the setup. */
  exitExplain: 'An exit is a member whose internet you use to open sites: sites see their address, not yours.',
  /** Row mark: the exit is chosen and working. */
  exitChosenOk: '✓ Your exit',
  /** Start of the Home panel text (same as in the tray). */
  routeBrowserPrefix: 'In the browser, sites open ',
  /** Start of the short route phrase in the header status line (browser): "Sites open directly" / "Sites open via <name>". */
  routeLinePrefix: 'Sites open ',
  /** Row mark: the exit is chosen but will not open sites right now. */
  exitChosenDown: 'Your exit — unavailable right now',
  /** Row mark: the member has not allowed opening sites through them. */
  exitRefused: 'Not available as an exit',
  /** Hint under the exit list on Home (single choice). */
  exitListHint:
    'Sites go through the one member you choose. If that member goes offline, sites may not open or may open directly until you choose another.',
  /** Step 1 of the Home setup after joining the network (without a network label). */
  wizardJoined: '✓ You joined the network',
  /** A field set at startup (`--cli-override`): the common part of the note. On Home, and for a field without an effective value, it is "… — can't be changed here"; in Settings, for a field with a value, it is "in file: … · currently in effect: … — set at startup…". */
  cliOverrideNote: 'set at startup',
  /** Connection strip: the page could not reach the node / the connection is back. */
  nodeNotResponding: `${NET_NAME} is not responding`,
  /** Connection strip: 401 — the start of the text (full: "… open it again: the Mesh Town icon near the clock → "Open Mesh Town…". Or paste the new link:"). */
  badTokenStrip: 'The page was opened from an old or incomplete link',
  /** Label of a member without a name: the column heading in the domain table (`member <id>`). */
  anonymousPeerPrefix: 'member ',
  /** Start of the own row's name in the member list. */
  selfPrefix: 'You: ',
} as const;

/** A member from the `/api/state` snapshot: what the label needs. */
export interface LabelPeer {
  id?: string;
  name?: string;
}

/**
 * Member label — like `peerLabel(p, withAnonymousTag, all)` in `common/words.ts`: with a name — the bare `name`, and `name (id)` only when
 * another member of `all` has the same name (case and edge spaces ignored); without a name — the bare id, and with `withAnonymousTag` —
 * `member <id>` (the heading of a domain table column).
 */
export function peerLabel(p: LabelPeer, withAnonymousTag = false, all: LabelPeer[] = []): string {
  const id = p.id ?? '';
  if (p.name && p.name.trim()) {
    const key = (o: LabelPeer): string => (o.name ?? '').trim().toLowerCase();
    return all.some((o) => o.id !== p.id && key(o) === key(p)) ? `${p.name} (${id})` : p.name;
  }
  return withAnonymousTag ? `${GLOSSARY.anonymousPeerPrefix}${id}` : id;
}

/** The hint above the member list while this computer is the only admin of the network (the id of its element is `#second-admin-hint`). */
export const SECOND_ADMIN_HINT =
  'You are the only admin of this network. Make another member an admin (⋯ → Make admin) so the network can still be managed if this computer is lost.';

/** Texts of the member-list part of the "Members" tab (removing, blocks, leaving, waiting to join). */
export const MEMBERS_WORDS = {
  menuBlock: 'Block on this computer…',
  menuRemove: 'Remove from network…',
  menuLeave: 'Leave network…',
  leaveTitle: 'Leave the network',
  leaveAction: 'Leave network',
  leftStatus: 'You left the network',
  leftTitle: /^You left the network/,
  dialogCancel: 'Cancel',
  removeAction: 'Remove',
  unblock: 'Unblock',
  badgeBlocked: 'Blocked on this computer',
  chipLabelOf: (n: number): string => (n === 1 ? '1 request needs your confirmation' : `${n} requests need your confirmation`),
  excludedSummary: (n: number): string => `No longer in the network (${n})`,
  removeTitle: (name: string, net: string): string => `Remove ${name} from "${net}"?`,
  blockTitle: (name: string): string => `Block ${name} on this computer?`,
  blockAction: (name: string): string => `Block ${name}`,
  moreActions: (name: string): string => `More actions for ${name}`,
  removedStatus: (name: string): string => `${name} was removed from the network.`,
  blockedStatus: (name: string): string => `${name} is blocked on this computer.`,
  excludedTitle: /^This computer was removed from the network/,
  excludedJoin: 'Join with another invite',
  pendingTitle: 'Waiting to join the network',
  badgePending: 'Waiting to join',
  admitAria: (name: string): string => `Confirm joining: ${name}`,
  labelLine: 'Invited as "Anna\'s laptop"',
  admitTitle: (name: string): string => `Confirm ${name} joining?`,
  admittedStatus: (name: string): string => `${name} joined the network.`,
} as const;
