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
  /** A field set at startup (`--cli-override`): the common part of the note. On Home, and for a field without an effective value, it is "… — can't be changed here"; in Settings, for a field with a value, it is "in file: … · currently in effect: … — set at node startup…". */
  cliOverrideNote: 'set at node startup',
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

/** Texts of the member-list part of the "Members" tab (ban votes, blocks, leaving). */
export const MEMBERS_WORDS = {
  menuBlock: 'Block on this computer…',
  menuBan: 'Ban from network…',
  menuLeave: 'Leave network…',
  menuWaiting: 'Waiting for another member to confirm',
  menuWithdraw: 'Withdraw proposal',
  reasonLabel: 'Reason (optional, members will see it)',
  leaveTitle: 'Leave the network',
  leaveAction: 'Leave network',
  leftStatus: 'You left the network',
  leftTitle: /^You left the network/,
  withdrawnStatus: 'Ban proposal withdrawn.',
  dialogCancel: 'Cancel',
  proposeBan: 'Propose ban',
  confirmBan: 'Confirm ban…',
  notNow: 'Not now',
  unblock: 'Unblock',
  badgeBanProposed: 'Ban proposed',
  badgeBlocked: 'Blocked on this computer',
  waitingLine: 'You proposed banning it · waiting for 1 more member to confirm',
  proposedByLine: 'Proposed by peer-one · needs 1 more confirmation',
  chipLabelOf: (n: number): string => (n === 1 ? '1 request needs your confirmation' : `${n} requests need your confirmation`),
  excludedSummary: (n: number): string => `No longer in the network (${n})`,
  banTitle: (name: string): string => `Ban ${name} from the network?`,
  blockTitle: (name: string): string => `Block ${name} on this computer?`,
  blockAction: (name: string): string => `Block ${name}`,
  moreActions: (name: string): string => `More actions for ${name}`,
  voteTitle: (by: string, target: string): string => `${by} proposes banning ${target} from the network`,
  banProposedStatus: (name: string): string =>
    `Ban proposed. ${name} is already blocked on this computer. Ask another member to open Mesh Town and confirm on the Members tab.`,
  reasonFrom: (by: string, reason: string): string => `Reason from ${by}: "${reason}"`,
  bannedStatus: (name: string): string => `${name} was banned from the network.`,
  blockedStatus: (name: string): string => `${name} is blocked on this computer.`,
  excludedTitle: /^This computer was banned from the network/,
  excludedJoin: 'Join with another invite',
  pendingTitle: 'Waiting to join the network',
  badgePending: 'Waiting to join',
  admitAria: (name: string): string => `Confirm joining: ${name}`,
  admissionLine: 'Confirmed by 0 of 1',
  labelLine: 'Invited as "Anna\'s laptop"',
  meWaiting: 'This computer is waiting for network members to confirm it (0 of 1).',
  admitTitle: (name: string): string => `Confirm ${name} joining?`,
  admittedStatus: (name: string): string => `${name} joined the network.`,
  menuForget: 'Remove from network…',
  menuConfirmForget: 'Confirm removal…',
  menuUndoForget: 'Undo removal',
  badgeForgetProposed: 'Removal proposed',
  forgetWaiting: 'Waiting for one more member to confirm',
  staleSummary: (n: number): string => `Offline for over a week (${n})`,
  forgottenSummary: (n: number): string => `Removed from the member list (${n})`,
  neverSeen: 'Never seen online',
  lastOnline: (ago: string): string => `Last online ${ago}`,
  forgetTitle: (name: string): string => `Remove ${name} from the network?`,
  forgetPropose: 'Propose removal',
  forgetConfirmTitle: (name: string): string => `Confirm removing ${name}?`,
  forgetAction: (name: string): string => `Remove ${name}`,
  forgetProposedStatus: (name: string): string =>
    `Removal proposed. ${name} is hidden on this computer. Ask another member to open Mesh Town and confirm on the Members tab.`,
  forgottenStatus: (name: string): string => `${name} was removed from the member list.`,
  unforgottenStatus: (name: string): string => `Removal withdrawn. ${name} is back in the member list.`,
  forgottenTitle: /^This computer was removed from the member list/,
  forgottenJoin: 'Join with an invite',
  forgottenStatusLine: /^Removed from the member list/,
} as const;
