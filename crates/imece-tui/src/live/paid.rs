//! imece's own paid calls (`:commit`'s commit messages, `:review`'s
//! reviews): each kind has a day's ceilings of its own, a count of calls
//! and of what they cost, kept in the store (`paid`, by kind and day) and
//! mirrored here for the day at hand. A call is counted at its cap, in
//! the store and in the pot, before it is made; what it cost replaces the
//! cap when it answers.

use super::*;
use crate::store::Paid;

/// A kind of paid call's day: how many calls and what they may cost
/// together (held caps counting), and the word its refusals use.
pub(super) struct Ceiling {
    pub kind: Paid,
    pub calls: u32,
    /// Ten-thousandths of a dollar.
    pub cost: u64,
    /// What the calls are called: `suggestions`, `reviews`.
    pub noun: &'static str,
}

/// The count of one kind for the day at hand: the day (days since the
/// epoch, UTC), its calls and what they cost or hold. A day other than
/// the one asked about is read from the store first.
pub(super) type Count = (u64, u32, u64);

/// Today, as the day ceilings count: days since the epoch, UTC.
pub(super) fn today() -> u64 {
    now() / 86_400
}

/// Whether the pot and `day` have room for one more call of `c`'s kind
/// that may cost up to `cap`; why not. `count` is read again from the
/// store when it is of another day.
pub(super) fn room(
    store: Option<&Store>,
    w: &World,
    count: &mut Count,
    c: &Ceiling,
    cap: u64,
    day: u64,
) -> Result<(), String> {
    if w.budget.saturating_sub(w.spent()) < cap {
        return Err(format!("the pot has less than {} left", dollars(cap)));
    }
    if count.0 != day {
        let had = store.map_or(Ok((0, 0)), |s| s.paid_on(c.kind, day));
        let (calls, cost) = had.map_err(|e| format!("its count could not be read: {e}"))?;
        *count = (day, calls, cost);
    }
    if count.1 >= c.calls {
        return Err(format!("today's {} {} were made", c.calls, c.noun));
    }
    if count.2 + cap > c.cost {
        return Err(format!(
            "today's {} for {} is spent",
            dollars(c.cost),
            c.noun
        ));
    }
    Ok(())
}

/// Counts one call of `c`'s kind on `day` at `cap`, in the store first
/// (one transaction checks and counts) and in the pot, before it is made.
pub(super) fn reserve(
    mut store: Option<&mut Store>,
    w: &mut World,
    count: &mut Count,
    c: &Ceiling,
    cap: u64,
    day: u64,
) -> Result<(), String> {
    room(store.as_deref(), w, count, c, cap, day)?;
    if let Some(store) = &mut store {
        let counted = store
            .reserve(c.kind, day, cap, c.calls, c.cost)
            .map_err(|e| format!("it could not be counted: {e}"))?;
        if !counted {
            return Err("the day's ceiling is reached".into());
        }
    }
    count.1 += 1;
    count.2 += cap;
    w.paid += cap;
    Ok(())
}

/// A call of `kind` counted on `day` at `cap` cost `cost`: the pot and the
/// day keep that instead of its cap.
pub(super) fn settle(
    store: Option<&mut Store>,
    w: &mut World,
    count: &mut Count,
    kind: Paid,
    day: u64,
    cap: u64,
    cost: u64,
) {
    w.paid = w.paid.saturating_sub(cap) + cost;
    if count.0 == day {
        count.2 = count.2.saturating_sub(cap) + cost;
    }
    if let Some(store) = store {
        // A failed write leaves the cap counted: more, never less.
        let _ = store.settle(kind, day, cap, cost);
    }
}
