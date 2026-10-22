//! Writing and withdrawing own list records on a replica.

use crate::net::{NetError, NetResult, PeerId, Replica};

use super::record::{decode, encode, key_for, AclKind, AclValue, Via, MAX_NOTE_CHARS};

fn check(r: &dyn Replica, v: &AclValue) -> NetResult<()> {
    if v.note.chars().count() > MAX_NOTE_CHARS {
        return Err(NetError::Config("list record note is too long".into()));
    }
    if v.kind == AclKind::Boot && v.subject != r.me() {
        return Err(NetError::Config("boot marker subject must be the own node".into()));
    }
    Ok(())
}

/// Whether the own live record under `key` is a valid record of this kind and subject and is not
/// a witness that the new record would turn into a vote.
async fn has_live(r: &dyn Replica, key: &[u8], v: &AclValue) -> NetResult<bool> {
    let Some(e) = r.get(r.me(), key).await? else {
        return Ok(false);
    };
    let same = decode(&e.value).is_ok_and(|old| old.kind == v.kind && old.subject == v.subject && !promotes(&old, v));
    Ok(!e.is_tombstone() && same)
}

/// A witness is overwritten by a record of another origin, never the other way round.
fn promotes(old: &AclValue, new: &AclValue) -> bool {
    old.via == Via::Witness && new.via != Via::Witness
}

/// Writes the own record under the key of its kind. `Ok(true)` — written; `Ok(false)` — the own live
/// record of the same kind and subject exists, the first one wins and nothing is written; the exception
/// is an own witness, which a record of another origin replaces.
pub async fn ensure(r: &dyn Replica, v: &AclValue) -> NetResult<bool> {
    check(r, v)?;
    let key = key_for(v.kind, &v.subject);
    if has_live(r, &key, v).await? {
        return Ok(false);
    }
    r.set_own(&key, encode(v).into()).await?;
    Ok(true)
}

/// Writes the own record over whatever lies under its key, without comparing with the old one.
pub async fn rewrite(r: &dyn Replica, v: &AclValue) -> NetResult<()> {
    check(r, v)?;
    r.set_own(&key_for(v.kind, &v.subject), encode(v).into()).await
}

/// `ensure` without the flag.
pub async fn put(r: &dyn Replica, v: &AclValue) -> NetResult<()> {
    ensure(r, v).await.map(|_| ())
}

/// Puts a tombstone on the own record. `Ok(false)` — no record or already a tombstone, nothing written.
pub async fn withdraw(r: &dyn Replica, kind: AclKind, subject: PeerId) -> NetResult<bool> {
    let key = key_for(kind, &subject);
    let live = r.get(r.me(), &key).await?.is_some_and(|e| !e.is_tombstone());
    if !live {
        return Ok(false);
    }
    r.del_own(&key).await?;
    Ok(true)
}
