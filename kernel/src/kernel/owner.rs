//! A content owner's local safe-area insets (@ref LLP 1123 D9).
//!
//! A native container that hosts Exact content (a route under a navigation
//! bar, a tab panel above a tab bar, a sheet under its grabber) clears its
//! own chrome, so a box inside it has an inset of its own that `env()`,
//! the document's, does not report. The host says each owner's insets on
//! the owner's root; `var(--exact-safe-area-inset-<edge>)` reads the
//! nearest one at or above a node, as an inherited custom property reads
//! its nearest declaration, and the session root's
//! ([`crate::Env::local`]) where no owner holds the node.
//!
//! The values change no layout by themselves: a length that reads one is
//! resolved per node where its engine style is derived, so the bars' room
//! is a length an author can pad with, or pull a background under.

use super::Kernel;
use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::generated::StyleProps;
use crate::id::{NodeFlags, ViewId};
use crate::layout::LayoutMirror;
use crate::style::{Dimension, Edge, Env, RowValue};
use std::borrow::Cow;

/// Whether a style holds a length that reads a local inset.
pub(crate) fn reads_local(style: &StyleProps) -> bool {
    style.mask.iter().any(
        |id| matches!(style.get(id), RowValue::Dimension(Dimension::Compare(c)) if c.reads_local()),
    )
}

/// The environment `slot`'s lengths, `style`, resolve against: the
/// arena's, unless they read a local inset under an owner ([`env_at`]).
pub(crate) fn env_for<'a>(arena: &'a NodeArena, slot: u32, style: &StyleProps) -> Cow<'a, Env> {
    match arena.blocks.has_locals() && reads_local(style) {
        true => env_at(arena, slot),
        false => Cow::Borrowed(arena.env()),
    }
}

/// The environment `slot`'s lengths resolve against: the arena's, with the
/// local insets of the nearest owner at or above `slot` in place of the
/// session root's.
pub(crate) fn env_at(arena: &NodeArena, slot: u32) -> Cow<'_, Env> {
    if !arena.blocks.has_locals() {
        return Cow::Borrowed(arena.env());
    }
    let mut at = Some(slot);
    while let Some(s) = at {
        if let Some(local) = arena.blocks.local(s) {
            let mut env = arena.env().clone();
            env.local = Some(local);
            return Cow::Owned(env);
        }
        at = arena.parent(s);
    }
    Cow::Borrowed(arena.env())
}

/// A box moved to another parent, created ones included, may be in another
/// owner now: what in its subtree reads a local inset is derived again.
/// Whether anything was.
pub(crate) fn moved(arena: &mut NodeArena, layout: &mut dyn LayoutMirror, slot: u32) -> bool {
    if !arena.blocks.has_locals() {
        return false;
    }
    let mut any = false;
    for s in arena.subtree(slot) {
        if !reads_local(&arena.effective_style(s)) {
            continue;
        }
        if let Some(node) = arena.taffy(s) {
            layout.restyle(arena, s, node);
            layout.mark_dirty(node);
        }
        arena.flags_mut(s).insert(NodeFlags::STYLE_DIRTY);
        any = true;
    }
    any
}

impl Kernel {
    /// Set or clear (`None`) the local insets of the content owner whose
    /// root is `view`, top right bottom left in points: what its container's
    /// chrome takes of its box, which `var(--exact-safe-area-inset-*)`
    /// reads in its subtree (LLP 1123 D9). Re-derives and dirties exactly the
    /// nodes there whose style reads one; whether any did (a layout is owed
    /// then). Nonfinite or negative insets are refused.
    pub fn set_local_insets(
        &mut self,
        view: ViewId,
        insets: Option<[f32; 4]>,
    ) -> Result<bool, KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        if !insets
            .unwrap_or([0.0; 4])
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0)
        {
            return Err(LayoutError::InvalidEnv.into());
        }
        if self.arena.blocks.local(slot) == insets {
            return Ok(false);
        }
        self.arena.blocks.set_local(slot, insets);
        let readers: Vec<u32> = (self.arena.iter_live())
            .filter(|&s| s == slot || self.arena.is_ancestor(slot, s))
            .filter(|&s| reads_local(&self.arena.effective_style(s)))
            .collect();
        self.restyle_readers(&readers);
        Ok(!readers.is_empty())
    }

    /// Set the session root's local insets (`None`: the safe-area insets,
    /// `env()`'s), which a node outside every owner reads: the insets of
    /// the area the document covers (LLP 1123 D9). A host whose document
    /// covers its view while `env()` is 0 sets them to the view's. Returns
    /// whether a layout is owed, as [`Kernel::set_env`] does.
    pub fn set_root_local_insets(&mut self, insets: Option<[f32; 4]>) -> Result<bool, KernelError> {
        let mut next = self.arena.env().clone();
        next.local = insets;
        if !next.is_finite() || insets.is_some_and(|l| l.iter().any(|v| *v < 0.0)) {
            return Err(LayoutError::InvalidEnv.into());
        }
        self.replace_env(next)
    }

    /// What `var(--exact-safe-area-inset-*)` reads at `view`, top right
    /// bottom left: its owner's local insets, or the session root's.
    pub fn local_insets(&self, view: ViewId) -> Option<[f32; 4]> {
        let slot = self.arena.slot_of(view)?;
        let env = env_at(&self.arena, slot);
        Some(Edge::ALL.map(|e| env.local_inset(e)))
    }

    /// The environment `view`'s lengths resolve against: the page's, with
    /// its owner's local insets (LLP 1123 D9). A host that resolves a row
    /// itself reads this one.
    pub fn env_of(&self, view: ViewId) -> Env {
        match self.arena.slot_of(view) {
            Some(slot) => env_at(&self.arena, slot).into_owned(),
            None => self.env(),
        }
    }

    fn restyle_readers(&mut self, readers: &[u32]) {
        if readers.is_empty() {
            return;
        }
        if let Some(r) = &mut self.region {
            r.invalidate();
        }
        for &slot in readers {
            if let (Some(node), Some(layout)) = (self.arena.taffy(slot), self.layout.as_deref_mut())
            {
                layout.restyle(&self.arena, slot, node);
                layout.mark_dirty(node);
            }
            self.arena.flags_mut(slot).insert(NodeFlags::STYLE_DIRTY);
        }
        // A replayed row checks its styles, not what they now resolve to
        if let Some(tree) = self.layout.as_deref_mut().and_then(|l| l.tree()) {
            tree.forget_memo();
        }
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
