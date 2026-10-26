/* When the "name a second admin" hint shows: this computer is an admin and no one else is. */

export const SECOND_ADMIN_HINT_ID = "second-admin-hint";

/** `onlyAdmin` is `meAdmin && adminIds.length === 1` from `managedFacts`; two admins or a plain member hide the hint. */
export const secondAdminHintShown = (onlyAdmin: boolean | undefined): boolean => !!onlyAdmin;
