import * as path from 'node:path';

import { e2eOutDir, reapOrphans } from './orphans';

// Before the run, removes targets left over from a killed run (the pid registry in <OUT>/pids).
export default function globalSetup(): void {
  const killed = reapOrphans(path.join(e2eOutDir(), 'pids'));
  console.log(`orphan cleanup: killed ${killed.length}${killed.length > 0 ? ` (${killed.join(', ')})` : ''}`);
}
