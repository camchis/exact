//! Share instance expressions after type and shape checks have seen substitution.
use crate::{expr, LowerError, Lowerer};
use contract_syntax::{
    inline::{Owner, Thunk},
    Expr, Span,
};
use contract_types::{Scope, Ty};
use exact_plan::{asm::Asm, ArmsId, DerivesId};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Sharing {
    thunks: Vec<Thunk>,
    by_span: BTreeMap<Span, Vec<usize>>,
    rows: Vec<Option<(DerivesId, Ty)>>,
    active: BTreeSet<usize>,
    pub root_scope: Scope,
    pub arm: Option<ArmsId>,
    pub instance: u32,
    pub regions: BTreeMap<u32, u32>,
    parents: Vec<Option<u32>>,
}

impl Sharing {
    pub fn new(ex: &contract_syntax::Expanded) -> Self {
        let thunks = &ex.thunks;
        let mut by_span = BTreeMap::<_, Vec<_>>::new();
        for (i, thunk) in thunks.iter().enumerate() {
            by_span.entry(thunk.expr.span()).or_default().push(i);
        }
        Self {
            thunks: thunks.to_vec(),
            by_span,
            rows: vec![None; thunks.len()],
            active: BTreeSet::new(),
            root_scope: Scope::default(),
            arm: None,
            instance: 0,
            regions: ex.region_instances.clone(),
            parents: ex.instances.iter().map(|i| i.parent).collect(),
        }
    }
}

impl Lowerer<'_> {
    pub(crate) fn shared_expression(
        &mut self,
        e: &Expr,
        scope: &Scope,
    ) -> Result<Option<(DerivesId, Ty)>, LowerError> {
        let Some(candidates) = self.sharing.by_span.get(&e.span()) else {
            return Ok(None);
        };
        let mut chain = vec![self.sharing.instance];
        while let Some(Some(parent)) = self.sharing.parents.get(*chain.last().unwrap() as usize) {
            chain.push(*parent);
        }
        let mut candidates = candidates.clone();
        candidates.sort_by_key(|i| {
            chain
                .iter()
                .position(|n| *n == self.sharing.thunks[*i].instance)
                .unwrap_or(usize::MAX)
        });
        let found = candidates.into_iter().find_map(|i| {
            if !chain.contains(&self.sharing.thunks[i].instance) {
                return None;
            }
            if self.sharing.active.contains(&i) || self.sharing.thunks[i].expr != *e {
                return None;
            }
            let (owner, own_scope) = match self.sharing.thunks[i].owner {
                Owner::Root | Owner::Instance => (None, self.sharing.root_scope.clone()),
                Owner::Arm { tag, arm } => {
                    let (id, scope) = self.arm_scopes.get(&(tag, arm))?;
                    (Some(*id), scope.clone())
                }
            };
            self.b.plan().frame_cut(self.sharing.arm, owner)?;
            // A callback or expression-local binder can shadow a prop's
            // argument name. Share only where every free name resolves as
            // it does in the instance frame.
            let cut = scope.region_depth().checked_sub(own_scope.region_depth())?;
            if !same_reads(e, scope, &own_scope, cut, &mut Vec::new()) {
                return None;
            }
            Some((i, owner, own_scope))
        });
        let Some((i, owner, own_scope)) = found else {
            return Ok(None);
        };
        if let Some(row) = &self.sharing.rows[i] {
            return Ok(Some(row.clone()));
        }
        let thunk = self.sharing.thunks[i].clone();
        let (arm, instance) = (self.sharing.arm, self.sharing.instance);
        self.sharing.active.insert(i);
        self.sharing.arm = owner;
        self.sharing.instance = thunk.instance;
        let mut asm = Asm::new();
        let result = expr::compile(self, &mut asm, &thunk.expr, &own_scope, &mut 0);
        self.sharing.active.remove(&i);
        self.sharing.arm = arm;
        self.sharing.instance = instance;
        let ty = result?;
        let type_id = self.ty_id(&ty)?;
        let body = self.b.code(asm);
        let row = self.b.derive(&thunk.name, type_id, body);
        self.b.set_derive_unsettled(row, owner);
        if let Some(sites) = &mut self.sites {
            sites.derives.push(crate::sites::Declared {
                name: thunk.name,
                span: thunk.span,
                instance: thunk.instance,
                component: None,
            });
        }
        self.sharing.rows[i] = Some((row, ty.clone()));
        Ok(Some((row, ty)))
    }
}

fn same_reads(e: &Expr, reader: &Scope, owner: &Scope, cut: u32, bound: &mut Vec<String>) -> bool {
    use contract_types::Ref;
    if let Expr::Ident(name, _) = e {
        if bound.contains(name) {
            return true;
        }
        let rebase = |r| match r {
            Ref::Item(d) => d.checked_sub(cut).map(Ref::Item),
            Ref::Index(d) => d.checked_sub(cut).map(Ref::Index),
            Ref::Bound(d) => d.checked_sub(cut).map(Ref::Bound),
            Ref::Local(_) => None,
            r => Some(r),
        };
        return reader.lookup(name).and_then(|(r, _)| rebase(r))
            == owner.lookup(name).map(|(r, _)| r);
    }
    match e {
        Expr::Let {
            name, value, body, ..
        } => {
            if !same_reads(value, reader, owner, cut, bound) {
                return false;
            }
            bound.push(name.clone());
            let yes = same_reads(body, reader, owner, cut, bound);
            bound.pop();
            yes
        }
        Expr::Arrow { params, body, .. } => {
            let n = bound.len();
            bound.extend(params.iter().cloned());
            let yes = same_reads(body, reader, owner, cut, bound);
            bound.truncate(n);
            yes
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            if !same_reads(subject, reader, owner, cut, bound)
                || !same_reads(none, reader, owner, cut, bound)
            {
                return false;
            }
            bound.push(var.clone());
            let yes = same_reads(some, reader, owner, cut, bound);
            bound.pop();
            yes
        }
        _ => e
            .operands()
            .all(|e| same_reads(e, reader, owner, cut, bound)),
    }
}
