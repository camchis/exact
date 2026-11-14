/-
Executable derive calls over the expression VM. The caller keeps its machine;
the callee starts fresh and shares the call and callback-work counters.
Roster-call allocation and extent bounds remain outside this model, as in Vm.
-/
import Contract.Vm

namespace Contract.VmRows

open Vm

structure Row where
  code : Code
  /-- The owning arm's region; none means the root. -/
  owner : Option Nat := none
  deriving Inhabited

/-- Frame regions, like Vm.Env.frames, are innermost first. -/
def ownerFrames (frames : List (Option Nat × Frame)) (owner : Option Nat) :
    Option (List (Option Nat × Frame)) :=
  match owner with
  | none => some []
  | some r =>
    let rest := frames.dropWhile (fun f => f.1 != some r)
    if rest.isEmpty then none else some rest

/-- Cutting selects the nearest owner and retains every enclosing frame. -/
theorem owner_frames_cut (r : Nat) (frame : Frame) (rest)
    (innerFrames : List (Option Nat × Frame))
    (outside : ∀ f ∈ innerFrames, f.1 ≠ some r) :
    ownerFrames (innerFrames ++ (some r, frame) :: rest) (some r) =
      some ((some r, frame) :: rest) := by
  induction innerFrames with
  | nil => simp [ownerFrames]
  | cons f fs ih =>
    have first := outside f (by simp)
    have tail : ∀ g ∈ fs, g.1 ≠ some r := fun g hg => outside g (by simp [hg])
    simpa [ownerFrames, first] using ih tail

/-- A reader's extra inner frames add exactly their count to an owner's
item, index or match-binding depth. -/
theorem frame_read_rebase (innerFrames owner : List (Option Nat × Frame)) (d : Nat)
    (read : Frame → Option α) :
    (((innerFrames ++ owner).map (·.2))[innerFrames.length + d]?).bind read =
      ((owner.map (·.2))[d]?).bind read := by
  simp [List.map_append, List.getElem?_append_right]

structure Caller where
  code : Code
  env : Vm.Env
  regions : List (Option Nat × Frame)
  machine : Machine
  deriving Inhabited

structure State extends Caller where
  callers : List Caller := []
  calls : Nat := 0
  listSteps : Nat := 0
  deriving Inhabited

def enter (s : State) (row : Row) (frames : List (Option Nat × Frame)) : State :=
  { code := row.code
    env := { s.env with frames := frames.map (·.2), params := [], writable := [] }
    regions := frames
    machine := .init
    callers := s.toCaller :: s.callers
    calls := s.calls + 1
    listSteps := s.listSteps }

def resume (s : State) (v : Value) (caller : Caller) (rest : List Caller) : State :=
  { toCaller := { caller with machine := { caller.machine with
      pc := caller.machine.pc + 1
      stack := v :: caller.machine.stack } }
    callers := rest
    calls := s.calls
    listSteps := s.listSteps }

inductive Result where
  | next (s : State)
  | done (v : Value) (fx : Vm.Effects)

/-- Calls and callback work have one budget across the entire evaluation. -/
def advance (rows : List (Option Row)) (s : State) : Except Trap Result := do
  let callbackEnd := s.machine.cbs.head?.any (fun c => c.stop == s.machine.pc)
  if !callbackEnd then
    match s.code[s.machine.pc]? with
    | some (.loadDerive i) =>
      match rows[i]?.bind id with
      | some row =>
        if s.calls + 1 > 65536 then
          throw (.unsupported "CallLimit: executable derive calls")
        if s.callers.length >= 64 then
          throw (.unsupported "CallDepth: executable derive calls")
        let frames ← match ownerFrames s.regions row.owner with
          | some fs => pure fs
          | none => throw Trap.badScope
        return .next (enter s row frames)
      | none => pure ()
    | _ => pure ()
  let work := if callbackEnd then
      s.machine.cbs.head?.map (fun c => if c.rest.isEmpty then 0 else 1) |>.getD 0
    else match s.code[s.machine.pc]?, s.machine.stack with
      | some (.map _), .list (_ :: _) :: _ => 1
      | some (.filter _), .list (_ :: _) :: _ => 1
      | _, _ => 0
  let steps := s.listSteps + work
  if steps > 65536 then throw (.unsupported "IterationLimit: list steps")
  let s := { s with listSteps := steps }
  match ← Vm.step s.code s.env s.machine with
  | .run m => return .next { s with machine := m }
  | .done v fx =>
    match s.callers with
    | [] => return .done v fx
    | c :: cs => return .next (resume s v c cs)

def runFrom (rows : List (Option Row)) : Nat → State → Except Trap (Value × Vm.Effects)
  | 0, _ => .error .outOfFuel
  | n + 1, s => do
    match ← advance rows s with
    | .done v fx => pure (v, fx)
    | .next s => runFrom rows n s

def run (rows : List (Option Row)) (code : Code) (env : Vm.Env)
    (regions : List (Option Nat) := []) (fuel : Nat := 1 <<< 22) :
    Except Trap (Value × Vm.Effects) :=
  runFrom rows fuel { code, env, regions := regions.zip env.frames, machine := .init }

theorem enter_fresh (s : State) (r : Row) (fs) :
    (enter s r fs).machine = Machine.init ∧
    (enter s r fs).env.params = [] ∧ (enter s r fs).env.writable = [] := by
  exact ⟨rfl, rfl, rfl⟩

theorem enter_shared (s : State) (r : Row) (fs) :
    (enter s r fs).calls = s.calls + 1 ∧
    (enter s r fs).listSteps = s.listSteps := by
  exact ⟨rfl, rfl⟩

theorem resume_preserves (s : State) (v : Value) (c : Caller) (cs) :
    (resume s v c cs).machine.locals = c.machine.locals ∧
    (resume s v c cs).machine.cbs = c.machine.cbs ∧
    (resume s v c cs).calls = s.calls ∧
    (resume s v c cs).listSteps = s.listSteps := by
  exact ⟨rfl, rfl, rfl, rfl⟩

theorem root_frames (fs) : ownerFrames fs none = some [] := rfl

/-- The row cannot see either a caller local or an intervening arm. -/
example :
    (match run [some { code := [.loadItem 0, .ret], owner := some 1 }]
      [.str "caller local", .bindLocal, .loadDerive 0, .dropLocal, .ret]
      { frames := [{ item := some (.str "inner") }, { item := some (.str "owner") }] }
      [some 2, some 1] 100 with
      | .ok (.str "owner", _) => true
      | _ => false) = true := by native_decide

/-- A fresh local stack makes a malformed row's local read fail. -/
example :
    (run [some { code := [.loadLocal 0, .ret] }]
      [.str "caller local", .bindLocal, .loadDerive 0, .ret] {} [] 100).isOk = false := by
  native_decide

/-- Depth is shared even when every row contains only another call. -/
example :
    (match run [some { code := [.loadDerive 0, .ret] }]
      [.loadDerive 0, .ret] {} [] 1000 with
      | .error (.unsupported "CallDepth: executable derive calls") => true
      | _ => false) = true := by native_decide


/-- Sibling calls consume one counter; returning does not replenish it. -/
example :
    (match runFrom [some { code := [.unit, .ret] }] 100
      { code := [.loadDerive 0, .pop, .loadDerive 0, .ret]
        env := {}, regions := [], machine := .init, calls := 65535 } with
      | .error (.unsupported "CallLimit: executable derive calls") => true
      | _ => false) = true := by native_decide

/-- Work already spent by a caller is charged before a callee's callback. -/
example :
    (match runFrom [some { code := [.unit, .list 1, .map 1, .loadLocal 0, .ret] }] 100
      { code := [.loadDerive 0, .ret]
        env := {}, regions := [], machine := .init, listSteps := 65536 } with
      | .error (.unsupported "IterationLimit: list steps") => true
      | _ => false) = true := by native_decide

end Contract.VmRows
