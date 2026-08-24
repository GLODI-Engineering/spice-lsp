# SPICE Netlist Grammar — ngspice, Xyce, a reference tool

Unified reference for building language tooling (parser, LSP) over SPICE-family
netlists. Language-agnostic on purpose — this describes *what* has to be
recognized/resolved, not how to implement it in any particular runtime.

Sources:
- ngspice 46 User's Manual (`ngspice-46-manual.md`, ~28k lines)
- Xyce 7.10 Reference Guide (`Xyce_RG.md`, ~29.8k lines)
- Xyce 7.10 Users' Guide (`Xyce_UG.pdf`, converted locally with `pdftotext`) —
  used specifically to close gaps the Reference Guide left open (title-line
  rule, numeric scale-factor table, node-name hierarchy separator, `.MPDE`)
- a reference tool — **not yet available**, see [§7](#7-a reference tool--tbd)

Every statement below is tagged:
- 🟢 **COMMON** — same syntax/semantics in ngspice and Xyce (verified against both manuals)
- 🔵 **NGSPICE** — ngspice-specific (including XSPICE extensions)
- 🟠 **XYCE** — Xyce-specific
- 🟡 **DIVERGENT** — exists in both but with meaningfully different syntax or semantics — read the sub-rows carefully, this is where a naive shared parser breaks

---

## 0. Why this isn't one grammar

Both simulators are SPICE3-lineage but neither implements "SPICE" as a single
fixed grammar — each ships **several distinct expression sub-languages**
depending on context, and a netlist's exact parse can depend on
simulator-mode flags that aren't in the netlist text itself.

**ngspice** has four separate expression grammars:
1. Compile-time (`.param`, `.func`, `{}` brace-expressions, subckt defaults)
2. Run-time behavioral (B/E/G-source expressions) — subtly different function set from (1)
3. Interactive/post-processing vector scripting (inside `.control`, operates on simulation output)
4. The `.control` block's own csh-like statement/control-flow language

**Xyce** has one unified expression grammar (§4) used almost everywhere
(`.param`, `.func`, B-sources, `.print`), but the *context* a given expression
appears in restricts what it may reference (solution variables allowed on
`.print` lines, forbidden on most `.param` lines — see §4.4).

**Compatibility modes are a hidden axis.** ngspice's `ngbehavior=ps|lt|hs|...`
flag (set in `.spiceinit`, not the netlist) changes number parsing (RKM
notation), comment characters, `.lib` semantics, and pulse-source argument
counts. A parser that only reads the `.cir` file cannot fully know which
dialect-of-a-dialect it's looking at unless it also inspects the init file —
worth deciding up front how the LSP handles this (flag it as ambiguous, let
the user pin a mode in settings, or default to strict/native mode).

---

## 1. Lexical Rules

| Rule | ngspice | Xyce |
|---|---|---|
| Full-line comment | 🟡 `*` in column 1 | 🟡 `*` in column 1 — same |
| End-of-line comment | 🟡 `$`, `//` (and `;` seen working in practice); `$` loses meaning under `ngbehavior=ps` | 🟡 `;` only, documented |
| Line continuation | 🟢 `+` as first non-blank char of next line (or Unix `\\` at line end) continues previous line | same — 🟢 |
| Continuation exceptions | 🔵 `.title`, `.lib`, `.include` do **not** support continuation | not documented as restricted |
| Case sensitivity | 🟢 keywords/node names case-insensitive in batch mode | 🟢 same (examples freely mix case) |
| Title line | 🟢 first physical line of the file is *always* the title (treated as a comment even without `*`), even if it looks like a statement; `.title <text>` can override in ngspice | 🟢 **confirmed via Xyce Users' Guide §4.1.3.1**: "The first line of the netlist is the title line... treated as a comment even if it does not begin with an asterisk... forgetting this is a common mistake and will probably result in a parsing error." Same rule as ngspice. |
| `.END`/`.end` | 🟢 mandatory, must be the last line | 🟢 "marks end of netlist file" — same role |
| Ground node | 🟢 must be `0`; `gnd` auto-converted to `0` in ngspice (disable via `set no_auto_gnd`) | 🟢 `0` reserved as ground |
| Illegal node-name chars | 🔵 `= % ( ) , [ ] < > ~` reserved (XSPICE port-syntax collision) | 🟠 any printable ASCII except whitespace/`( ) { } , : ; " '`; extra rule: arithmetic-operator chars (`% ^ & ? : ~ * - + < > / \|`) and leading `#` are unsafe *inside expressions* specifically |
| Device name | 🟢 first char is a letter A–Z selecting the device type (R, C, L, V, I, D, Q, M, J, X, B, E, F, G, H, K, S, W, T, U, Z, ...) | 🟢 same convention, plus Xyce's `Y<type>` and `P` device letters |
| Global nodes | 🔵 via `.global` statement only | 🟡 automatic for any node starting with `$G`, **or** via `.global` (HSPICE-compat alias) |
| Reserved identifiers (can't be param names) | 🔵 `time, temper, hertz` + math function names | 🟠 `Time, Freq, Hertz, Vt, Temp, Temper, GMIN` |
| Malformed leading char | 🔵 ngspice auto-comments a line starting with `=[]?()&%$§"!:` and warns (fatal under `strict_errorhandling`) | not documented |

**LSP implication:** comment-stripping and continuation-joining must happen
*before* tokenizing statements — implement this as a line-preprocessing pass
per dialect, not inside the main grammar.

---

## 2. Numbers & Units

🟡 **DIVERGENT in one specific way (see `X` below), otherwise COMMON —
confirmed against both simulators' own scale-factor tables.**

ngspice's manual (§2.1.3.3) suffix table:

| Suffix | Factor |
|---|---|
| `T` | 1e12 |
| `G` | 1e9 |
| `Meg`/`MEG` | 1e6 |
| `K`/`k` | 1e3 |
| `mil` | 25.4e-6 |
| `m` | 1e-3 |
| `u` | 1e-6 |
| `n` | 1e-9 |
| `p` | 1e-12 |
| `f` | 1e-15 |
| `a` | 1e-18 |

Xyce Users' Guide Table 4-1 (confirmed by direct PDF read — the Reference
Guide alone doesn't state this table, but the Users' Guide does):

| Symbol | Equivalent value |
|---|---|
| `T` | 1e12 |
| `G` | 1e9 |
| `Meg` | 1e6 |
| `X` | 1e6 |
| `K` | 1e3 |
| `mil` | 25.4e-6 |
| `m` | 1e-3 |
| `u` (µ) | 1e-6 |
| `n` | 1e-9 |
| `p` | 1e-12 |
| `f` | 1e-15 |

🟢 **Confirmed common**: the two tables agree on every suffix ngspice
documents except `a` (atto, not in Xyce's table — matches the earlier
finding that Xyce needs `-hspice-ext units` to interpret `a` as atto at
all; by default `a` is not a recognized scale suffix in Xyce).

🟡 **One real divergence**: **Xyce accepts `X` as an alias for `Meg` (1e6)**,
per its own Users' Guide table — this suffix does not exist in ngspice's
table at all. A netlist using `1X` for `1e6` parses fine in Xyce and would
be a syntax error (or an unrecognized-suffix warning, decorative-letter
fallback) in ngspice. Worth a dialect-aware completion/validation rule.

Key disambiguation rule (🟢 COMMON, classic SPICE convention, confirmed in
both): **`m`/`M` is always milli** (1e-3); mega must be spelled `Meg`/`MEG`
(or `X` in Xyce). Trailing alphabetic characters after a number or after a
valid suffix are decorative and ignored: `10`, `10V`, `10Hz`, and `10Volts`
are the same value; `1000 == 1k == 1.0e3 == 1kHz`.

- 🟠 **Xyce-specific:** complex numbers via `J` suffix — `.param a0=1.0+2.0J`.
- 🔵 **ngspice-specific:** RKM notation (`2K7`, `4R7`) accepted only under
  `ngbehavior=lt` (a reference tool compatibility mode) — this is likely the *real*
  a reference tool number-parsing rule, worth confirming once a reference tool docs arrive.
- 🟠 **Xyce-specific:** `a` (atto, 1e-18) is **not** enabled by default —
  bare `a` normally reads as amperes-suffix noise; `-hspice-ext units`
  command-line flag re-enables the atto interpretation.
- 🟠 **Xyce-specific:** `-hspice-ext math` flips `log()` to mean natural log
  (HSPICE convention); by default Xyce's `LOG`/`LOG10` = base-10 and `LN` =
  natural log, whereas PSpice's `log()` = natural log (documented
  incompatibility Xyce explicitly calls out).

---

## 3. Element / Device Instance Syntax

🟢 **COMMON core** — both simulators use `<Letter><name> <nodes...> <params...>`,
first-letter selects device type, and the great majority of two-terminal
passive/source devices share identical field order.

### 3.1 Common device table (syntax verified equivalent in both manuals)

| Letter | Device | Form |
|---|---|---|
| R | Resistor | `R<name> n+ n- [model] <value\|R={expr}> [tc1=][tc2=][temp=][m=]` |
| C | Capacitor | `C<name> n+ n- [model] <value\|C={expr}\|Q={expr}> [ic=][tc1=][tc2=]` |
| L | Inductor | `L<name> n+ n- [model] <value\|L={expr}> [ic=][tc1=][tc2=]` |
| K | Mutual inductor | `K<name> L<ind1> L<ind2> [...] <coupling> [model]` |
| V | Independent voltage source | `V<name> n+ n- [[DC]value] [AC mag [phase]] [transient-fn]` |
| I | Independent current source | `I<name> n+ n- [[DC]value] [AC mag [phase]] [transient-fn]` |
| D | Diode | `D<name> anode cathode model [area]` |
| Q | BJT | `Q<name> c b e [subst] model [area]` |
| J | JFET | `J<name> d g s model [area]` |
| Z | MESFET | `Z<name> d g s model [area]` |
| M | MOSFET | `M<name> d g s bulk model [L=][W=][AD=][AS=]...` |
| E | VCVS | `E<name> n+ n- nc+ nc- gain` (+ VALUE/TABLE/POLY forms, §5) |
| G | VCCS | `G<name> n+ n- nc+ nc- transconductance` (+ VALUE/TABLE/POLY, §5) |
| F | CCCS | `F<name> n+ n- Vctrl gain` (+ POLY) |
| H | CCVS | `H<name> n+ n- Vctrl transresistance` (+ POLY) |
| B | Behavioral source | `B<name> n+ n- <I=expr\|V=expr>` (§5) |
| S | Voltage-controlled switch | `S<name> n+ n- nc+ nc- model [ON][OFF]` |
| W | Current-controlled switch | `W<name> n+ n- Vctrl model [ON][OFF]` |
| T | Lossless transmission line | `T<name> p1+ p1- p2+ p2- Z0=val [TD=][F=][NL=]` |
| X | Subcircuit call | `X<name> [nodes...] subckt_name [params...]` |

Transient source functions on V/I (🟢 same names/arg order in both, minor
default-value wording differences):
`PULSE(V1 V2 TD TR TF PW PER)`, `SIN(V0 VA FREQ TD THETA PHASE)`,
`EXP(V1 V2 TD1 TAU1 TD2 TAU2)`, `PWL(t1 v1 t2 v2 ...)`, `SFFM(V0 VA FC MDI FS)`.

### 3.2 Divergent / dialect-specific devices

🔵 **ngspice-only:**
- `A<name>` — XSPICE code models (analog/digital/hybrid blocks, §6.1)
- `N<name>` — Verilog-A/OSDI compact models (separate loader, not XSPICE)
- `O<name>` — lossy transmission line (LTRA)
- `U<name>` — dual meaning: uniform-RC line **or** XSPICE digital primitive, disambiguated by argument shape
- `P<name>` — coupled multiconductor line (KSPICE-derived)
- `Y<name>` — single lossy line (TXL, KSPICE-derived)
- `.probe` auto-instrumentation directive

🟠 **Xyce-only:**
- `P<name>` — **Xyce's `P` is a Port device** (S-parameter port) — ⚠️ letter
  collision with ngspice's `P` (coupled multiconductor line). Same letter,
  unrelated device — a shared parser must key device semantics off
  `(dialect, letter)`, never `letter` alone.
- `O<name>` — Xyce's `O` is also lossy transmission line (LTRA) — this one matches ngspice.
- `Y<type>` family (deprecated) and its modern replacements: `YACC`
  (accelerated mass), `YDELAY` (ideal delay), `YLIN` (linear S/Y/Z device),
  `YMEMRISTOR`, `ytransline`, `YPDE` (TCAD)
- `U<name>` in Xyce = behavioral digital device (single model card for timing
  + I/O) — **different semantics from ngspice's `U`**, another same-letter collision
  (`U<name> <type>(<numinputs>) pwr gnd in* out* model`)
- Power-grid devices: `PowerGridBranch`, `PowerGridBusShunt`,
  `PowerGridTransformer`, `PowerGridGenBus`
- BJT optional substrate node must be bracketed if non-numeric —
  `Q6 VC 4 11 [SUB] LAXPNP` — to disambiguate from a model name

**⚠️ Same-letter, different-device collisions found: `P`, `U`, and (once
a reference tool is in scope) very likely more. A dialect-agnostic core grammar must
never assume a device-letter table is shared — it must be a per-dialect
lookup.**

### 3.3 Multiplicity factor `m`

🟡 **DIVERGENT support surface.** ngspice supports `m=` on C, D, F, G, I, J,
L, M, Q, R, X, Z (and it propagates recursively through subcircuit calls).
Xyce supports `m=`/`M=` on R, L, C, MOSFET, some BJT models (VBIC 1.3,
MEXTRAM), B-source (current form only), and G-source — Xyce's own docs note
`m`/`s` hierarchical parameters are "not commonly supported," i.e. narrower
and less consistent than ngspice's.

---

## 4. `.PARAM` and Expression Grammar — the hard part

### 4.1 `.PARAM` statement

🟢 **COMMON surface syntax:**
```
.PARAM <ident>=<expr> [<ident>=<expr> ...]
```
Both wrap expressions in `{}` or `'...'` by convention; both tolerate bare
unquoted expressions on `.param` lines specifically, with caveats (ngspice:
`.param a = 123 * 3` silently truncates to `a=123` without quoting; Xyce:
bare expressions are "fragile," whitespace-sensitive around ternary `?:`).

🟡 **DIVERGENT specifics:**

| | ngspice | Xyce |
|---|---|---|
| Function-valued params | separate `.func` statement only | `.param SUM(A,B,C)={A+B+C}` — `.param` itself can define functions, equivalent to `.func` |
| Self-reference / recursion | 🔵 illegal — `.param pip='pip+3'` errors, must be assigned once, top-to-bottom, before first use | not documented as restricted the same way |
| Time/solution-variable dependence | 🔵 `.param` is purely compile-time (parser #1) — cannot reference `V()`/`I()`/simulation state at all | 🟠 **`.param` values can change during the calculation** (may depend on `TIME`, `FREQ`, `TEMP`, `VT`) but still may **not** depend on solution variables (node voltages/currents) except via specific device-parameter exceptions (§4.4) |
| String params | supported, concatenation-only, top-level `.param` lines only | not documented in the extraction — verify |
| Redefinition | not documented as tolerated | last-defined wins by default; tunable via `-redefined_params` CLI flag (`ignore\|uselast\|error\|warn\|...`) |
| Complex values | not applicable | `J` suffix imaginary part; feeding a complex param into a real device param silently drops the imaginary part |

### 4.2 Scoping

🟡 **DIVERGENT in mechanism, similar in effect.**

- 🔵 **ngspice**: lexical/shadowing scoping. Top-level `.param` = global.
  Inside a `.subckt`, formal params (`ident=value` after the node list) and
  local `.param` lines shadow same-named globals until `.ends`. "Assigning"
  a global-named param inside a subckt actually creates a local shadow —
  true mutation of a global from inside a subckt is impossible. Nesting
  depth up to 10 levels, innermost wins.
- 🟠 **Xyce**: same top-level-visible-everywhere / subcircuit-local-visible-downward
  pattern for `.param`, `.model`, `.func` — but Xyce additionally has
  **`.global_param`**, a stricter sibling of `.param`:
  - can only be declared at top-level circuit scope (never inside a subckt)
  - cannot define functions with arguments (unlike `.param`)
  - **cannot be redefined across hierarchy levels** — illegal, unlike
    `.param`'s tolerated same-scope redefinition
  - may only depend on top-level-scoped `.param` values
  - is the thing `.step`/`.sampling`/`.embeddedsampling` can sweep across
    the whole hierarchy
  - **Note:** the Xyce RG has **no `PARHIER=LOCAL|GLOBAL` keyword** (that's
    an HSPICE mechanism). Xyce's model is lexical scoping plus this one
    explicit `.global_param` escape hatch — don't build an LSP feature
    around a `PARHIER` keyword that doesn't exist in this Xyce version.
  - Only *globally-scoped* `.param`/`.global_param` can be swept by
    `.step`/`.sampling`/`.embeddedsampling`; subcircuit-local `.param`s can't
    be swept directly (only indirectly via dependency on a global param).
    This restriction doesn't apply to device instance/model parameters.

### 4.3 Operators & functions

🟡 **DIVERGENT — different operator vocab, similar precedence shape.**

| | ngspice (parser #1, compile-time) | Xyce (unified expression grammar) |
|---|---|---|
| Arithmetic | `+ - * / % \` (`\` = integer divide) | `+ - * / % **` |
| Power | `** ^` (both = `pwr`-like) | `**` only (no `^` for power — see below) |
| Boolean | `&& \|\|` `!` | `& \| ^` (`^` here = **boolean XOR**, not power!) `~` (unary NOT) |
| Comparison | `== != <= >= < >` | `== != > >= < <=` |
| Ternary | `c?x:y`, precedence level 8 (loosest) | `c?x:y`, "very low precedence, same as C" — **documented gotcha**: `1+a==b?1:0+1` parses as `IF(1+a==b, 1, 0+1)`, chained ternaries nest right-associatively in a way users find surprising |
| String concat | via `.param` string params only | `+` operator does double duty: numeric add **or** string concat |

**⚠️ `^` is a false cognate between the two dialects**: ngspice's compile-time
parser uses `^` as a power-operator alias; Xyce uses `^` for boolean XOR. A
shared "generic SPICE expression" grammar cannot treat `^` the same way
across dialects — it must be dialect-parameterized.

**⚠️ Xyce ternary/colon collision**: because `:` is Xyce's subcircuit-hierarchy
path separator (`V(Xmain:Xnot1:A)`) and *also* the ternary's separator, a bare
parameter name must never sit directly left of `:` — `{(A==B)?C:D}` is a
**syntax error** in Xyce; must write `{(A==B)?C :D}` or `{(A==B)?(C):D}`.
This is a real lint-diagnostic opportunity for the LSP.

Built-in math functions — 🟢 large common core (`sqrt, sin, cos, tan, asin,
acos, atan, sinh, cosh, tanh(*), exp, ln, log, abs, min, max, pow`), with
notable 🟡 divergences:
- `log`/`log10`: ngspice's `log` = natural log always (`log10` for base-10).
  **Xyce's `log`/`log10` = base-10 by default**, `ln` = natural log — this
  flips the meaning of the single most commonly typed function name between
  the two dialects, and again vs. PSpice (`-hspice-ext math` toggles Xyce to
  natural-log `log`, which is its own further variant).
- `pow`/`**`/`^`/`pwr` sign handling: in ngspice's *runtime* B-source parser
  (parser #2, distinct from parser #1!), `pow(x,y)`/`x**y`/`x^y` all discard
  the sign of `x` (`pow(fabs(x),y)`), while `pwr(x,y)` preserves sign. Xyce
  has an analogous sign-corrected `PWRS(x,y)` alongside plain `POW`/`PWR`.
- ngspice's runtime B-source function list explicitly **lacks `tanh`** (only
  present in parser #1); has `u`/`u2`/`uramp` (unit-step family) with no
  direct Xyce equivalent — Xyce's closest analog is `STP(x)`/`URAMP(x)`.
- Xyce-only: `DDT(x)`/`SDT(x)` (time derivative/integral — no direct ngspice
  compile-time equivalent, since ngspice's `.param` grammar is static and its
  B-source grammar has no derivative/integral primitive either), `DDX(f,x)`
  (partial derivative), `ATAN2`, `SGN`/`SIGN`, `NINT`, complex-number ops
  `DB/IMG/PH/R/RE`.
- ngspice-only (compile-time parser): `gauss/agauss/unif/aunif/limit(2-arg)/var/vec`
  as *compile-time* Monte-Carlo helpers — Xyce has near-identical
  `AGAUSS/GAUSS/AUNIF/UNIF/LIMIT(2-arg, also random)/RAND`, so this one is
  effectively 🟢 common in spirit, 🟡 divergent only in exact function set.

### 4.4 Where expressions may reference simulation state

🟡 **DIVERGENT rules, same underlying concern.**

- 🔵 ngspice: hard separation by *parser identity* — parser #1 (`.param`,
  `.func`, braces) never sees solution variables; parser #2 (B/E/G-source)
  always can (`V()`, `I()`, `time`, `temper`, `hertz`); parser #3
  (`.control` scripting) only sees post-simulation vectors, bridgeable to
  parser #1's numbers via `.csparam`.
- 🟠 Xyce: one grammar, but *context* gates what's legal:
  - `.param`/`.global_param` expressions: may reference other params, `TIME`,
    `FREQ`, `TEMP`, `VT` — **not** solution variables or lead currents.
  - `.print` line expressions: least restricted — params, device params
    (`<device>:<param>`), **and** solution variables.
  - Device instance/model parameters: generally must be time-independent
    constants — **except** a specific allow-list that may depend on
    time and/or solution variables: B-source `V=`/`I=`, switch `CONTROL=`,
    capacitor `C=`/`Q=`, linear mutual-inductor coupling, and (time-only,
    not solution-variable) `TEMP`, `L`, `R`, thermal-resistor params.

This exception list (Xyce §2.2.1–2.2.4) is exactly the kind of thing an LSP
should turn into a diagnostic: "expression references V(...) here, but this
parameter position does not allow solution-variable dependence in Xyce."

### 4.5 `.FUNC`

🟢 **COMMON**, near-identical:
```
.func <ident>(args) {<expr>}      ; or  .func <ident>(args) = {<expr>}
```
🟡 minor divergence: ngspice's `=` is effectively optional/either form works;
Xyce documents `.func` requiring `=` to be *optional* but `.param`-as-function
requiring `=` to be *mandatory*; Xyce also explicitly forbids functions from
shadowing built-ins or using `EXP`/`PI` as argument names (reserved math
constants) — not documented as a restriction in ngspice.

---

## 5. Behavioral Sources (B, E, G, POLY) — the other hard part

### 5.1 B-source

🟢 **COMMON base form:**
```
B<name> n+ n- <I=expr | V=expr> [device params]
```
Both: exactly one of `I=`/`V=` required; both support `tc1`/`tc2`-style
temperature coefficients and a `temp=`/`dtemp=` override.

🟡 **DIVERGENT — table/file lookups:**
- 🔵 ngspice: `pwl(<expr>, x0,y0, x1,y1, ...)` callable *inside* any B/E/G
  expression; extrapolates **linearly** past the table's x-range (add a
  padding point to fake clamping).
- 🟠 Xyce: `TABLE {expr} = (x0,y0)(x1,y1)...` and file-backed variants
  `table("file")`/`tablefile("file")`/`fasttable(...)`/`spline("file")`
  (Akima)/`cubic(...)`/`wodicka(...)`/`bli(...)` (Barycentric Lagrange) —
  richer interpolation choice than ngspice, and Xyce's inline `TABLE` form
  **clamps** at the boundary rather than extrapolating (opposite default
  behavior from ngspice's `pwl()`).

🟡 **Ternary/conditional:**
- ngspice: `c ? x : y`, documented gotcha — needs a space before `?` or the
  parser can misparse the token.
- Xyce: `c ? x : y` **and** `IF(c,x,y)` as an explicit function alias; low
  ternary precedence and the `:`-collision issue from §4.3.

🟡 **Smoothing / convergence:**
- 🟠 Xyce-only documented mitigation: `smoothbsrc=1` instance param (or
  `.options device smoothbsrc=1`) inserts an internal RC network to soften
  hard `IF()`/ternary transitions and avoid "timestep too small" failures.
  ngspice has no equivalent named knob in the extracted material (its
  general convergence aids are `.options gminsteps`/`srcsteps` etc., not
  B-source-specific).

🟡 **Frequency-domain support:**
- Both: B-sources referencing `TIME` don't work in AC-type analyses; purely
  solution-dependent B-sources do. Xyce explicitly documents this limitation
  applying uniformly to B/E/F/G/H; ngspice's `hertz` special variable exists
  specifically to let a B-source depend on AC frequency (at the cost of
  forcing an OP recompute per frequency point) — this is a capability Xyce's
  docs don't describe an equivalent for.

### 5.2 E/G sources — VALUE/TABLE/POLY forms

🟢 **COMMON shape**, both simulators offer three syntactic sugars over the
same underlying B-source-equivalent machinery:
```
E<name> n+ n- VALUE = {expr}                              ; or ngspice: vol='expr'
E<name> n+ n- TABLE {expr} = (x0,y0)(x1,y1)...
E<name> n+ n- POLY(N) nc1+ nc1- ... p0 p1 ...
```
(G-source mirrors E with `cur=`/current semantics.)

🟡 **DIVERGENT:**
- ngspice additionally supports `LAPLACE`/`FREQ` E-source forms and
  `AND/OR/NAND/NOR` logic-style E-sources — **internally rewritten to XSPICE
  code models** (`s_xfer`, `xfer`, `multi_input_pwl`). Xyce's docs explicitly
  state it does **not** support PSpice's `FREQ`/`LAPLACE`/`CHEBYSHEV` E/G
  forms — so this is a real capability gap, not just syntax, when converting
  netlists between the two.
- Xyce warns that PSpice-style `TABLE {EXPR} ((x1,y1)...)` (extra parens, no
  `=`) or `TABLE {EXPR} = (x1 y1)...` (space instead of comma) are **not**
  legal Xyce syntax — must normalize to `TABLE {EXPR} = (x1,y1)(x2,y2)...`.
  A netlist-format converter/linter should check for this specifically.

### 5.3 POLY

🟢 **COMMON**, same three-shape pattern (E/G take node pairs, F/H take
controlling-source names, B takes explicit named variables), same "no
performance benefit over writing the expression directly, provided for
legacy-netlist compatibility" framing in both.

---

## 6. Structural / Container Statements

### 6.1 `.SUBCKT` / `.ENDS`

🟢 **COMMON:**
```
.SUBCKT <name> [node]* [PARAMS:] [<ident>=<value>]*
  ...
.ENDS [<name>]
```
```
X<name> [node]* <subckt-name> [PARAMS:] [<ident>=<value>]*
```
Both: node `0`/ground cannot appear in the subckt's own node list; call-site
node count must match definition; nesting of both *definitions* (a
`.subckt` inside another) and *references* (subckt A instantiates subckt B)
is supported to arbitrary/bounded depth.

🟡 **DIVERGENT:**
- 🟠 Xyce explicitly forbids a subcircuit calling itself, directly or
  transitively (circular reference is a documented error). ngspice's manual
  doesn't state an explicit circularity check as a named rule — but it does
  state that subcircuit instantiation is implemented as **pure textual
  substitution** ("each subcircuit instance is replaced by its definition
  using text expansion... the hierarchy is not present after input
  processing," §2.6): a circular reference would make that substitution
  never terminate, so it's illegal *by construction* even without a named
  "no circular references" rule — expect an expansion-depth/hang failure
  mode rather than a clean diagnostic message. An LSP should still detect
  and flag circular subckt references explicitly rather than relying on
  ngspice's own (non-existent) error message for it.
- 🟢 **Confirmed — `PARAMS:` is Xyce/HSPICE-specific, not ngspice syntax.**
  Grepped the full ngspice-46 manual for the literal token `PARAMS:`: zero
  occurrences. ngspice's own §2.11.3 examples use bare `ident=value` after
  the node list with **no** `PARAMS:` keyword at all — it is not an accepted
  alias, just absent from the grammar. Xyce's `PARAMS:` is a keyword some
  other SPICE dialects (HSPICE, PSpice) require but Xyce treats as
  optional. **Do not accept a `PARAMS:` token in an ngspice-mode parser.**
- 🟡 **Confirmed divergent — name-mangling separator differs.** Xyce's
  default hierarchy separator is `:` (`X3:Q17`, `V(Xmain:Xnot1:A)`),
  changeable to `.` via the `-hspice-ext separator` command-line flag.
  ngspice's manual (§13, "Node voltages and branch currents from within a
  subcircuit," and its `save`/`alterparam` examples) confirms ngspice uses
  **`.` (dot) as its hierarchy separator**, not `:` — e.g.
  `save x1.x1.x1.7 v(9)`, extended node names like `xsub1.int1` or
  `xsub1.xsub2.int2`. So by default the two dialects use **different**
  separator characters for the exact same concept — a hard-coded `:` or `.`
  assumption anywhere in symbol resolution / go-to-definition logic must be
  dialect-parameterized, not shared.

### 6.2 `.MODEL`

🟢 **COMMON:**
```
.MODEL <name> <type> ( <param>=<value> ... )
```
Both key complex compact models (BSIM, VBIC, etc.) by a `LEVEL=` (and
sometimes `VERSION=`) sub-selector, and both support **model binning**
(same base name + suffix, differentiated by `lmin/lmax/wmin/wmax` for
geometry-dependent parameter sets, MOSFET-oriented).

🟡 **DIVERGENT:**
- 🟠 Xyce: parentheses optional but if present must be a *single* pair
  around the whole list — explicitly rejects PSpice-style partial/nested
  parenthesization (`Is=1e-16 (Xti=3 Bf=100)` is illegal) and rejects
  comma-separated parameter lists.
- 🟡 `LEVEL` numbers are **not portable between dialects** — e.g. BSIM3 is
  LEVEL=9 in Xyce but LEVEL=7 in PSpice; ngspice has its own level-numbering
  tradition again. A "go to model definition" LSP feature must resolve
  `LEVEL` meaning per-dialect, never assume a shared numbering.
- 🔵 ngspice: 17 base model `type` codes enumerated explicitly (`R, C, L,
  SW, CSW, URC, LTRA, D, NPN, PNP, NJF, PJF, NMOS, PMOS, NMF, PMF, VDMOS`).
  Xyce's type vocabulary is larger due to Y-devices/power-grid devices
  needing their own model types — no single shared enum.

### 6.3 `.INCLUDE` / `.LIB`

🟡 **DIVERGENT in `.lib` semantics, common in `.include`.**
```
.INCLUDE <filename>        ; 🟢 common — textual splice, searched via a path list
.LIB <filename> <section>  ; select one named section out of a multi-section library file
```
- 🔵 ngspice: `.lib`/`.include`/`.title` do not support `+` continuation.
  Under `ngbehavior=ps`, a bare `.LIB filename` (no section name) degrades
  to a plain include with a warning. ngspice-only `.INCPSLT` variant forces
  PSLT compat mode for that one included file regardless of the outer mode.
- 🟠 Xyce: three synonymous spellings — `.INC`/`.INCLUDE`/`.INCL`. `.LIB`
  has two forms (call-site `.lib file section` vs. in-file
  `.lib section ... .endl section` definition) and Xyce's docs explicitly
  say **"the Xyce version of `.LIB` has been designed to be compatible with
  HSPICE, not PSpice."** Three-tier search path: relative to including
  file → relative to top-level netlist → relative to Xyce's own execution
  directory.

### 6.4 `.GLOBAL`, `.IC`, `.NODESET`, `.OPTIONS`

🟡 **DIVERGENT, especially `.options`.**

- `.global <node1> [node2 ...]` — 🟢 common statement form. (Xyce also has
  the implicit `$G`-prefix mechanism as an alternative, §1.)
- `.ic v(node)=value ...` — 🟢 common form and intent (enforced initial
  condition, used with `uic`/`NOOP`-style skip-DCOP flags). 🟠 Xyce
  explicitly disallows `.ic`/`.nodeset` **inside subcircuits at all**
  (documented DCOP-failure gotcha vs. PSpice) — must be at top level with
  fully-qualified hierarchical node names. ngspice's extraction didn't
  surface the same restriction; confirm before assuming parity.
- `.nodeset` — 🟢 common as a convergence-aid-only (non-enforced) initial
  guess, contrasted with `.ic`'s enforced constraint, in both dialects.
- `.options` — 🟡 **structurally different**: 🔵 ngspice uses one flat
  `.options opt=val ...` namespace (with a documented precedence chain:
  hardcoded default < `spinit` < in-netlist `.options` < interactive
  `.control` `option` command). 🟠 Xyce has **no flat/global `.options`
  line at all** — every settings group requires its own package keyword,
  `.OPTIONS <PKG> name=value ...` (`DEVICE`, `TIMEINT`, `NONLIN`, `LINSOL`,
  `OUTPUT`, `PARSER`, `SENSITIVITY`, `HBINT`, `MEASURE`, ...), and Xyce's
  `.options` statements are **top-level only** (inside a subckt → ignored
  with a warning). Porting an ngspice `.options` line to Xyce requires
  knowing which package each option name belongs to — not a 1:1 syntax
  translation.

---

## 7. Analysis Statements

🟡 **DIVERGENT beyond the shared basics — read carefully, this is a common
source of silently-wrong netlist ports between the two.**

🟢 **Common core** (same statement name, compatible core arguments):
```
.AC  <LIN|OCT|DEC> <points> <fstart> <fstop>
.DC  <src> <start> <stop> <step> [<src2> <start2> <stop2> <step2>]
.OP
.TRAN <step> <tstop> [<tstart> [<tmax>]] [UIC]
```

🟡 **`.TRAN` first-argument semantics differ** — this is a real trap, not
just cosmetic: Xyce's docs explicitly flag that Xyce is "not strictly
compatible with SPICE" here — in classic SPICE/ngspice the first `.TRAN`
number is the *print interval*; in Xyce it's the *initial timestep* used for
step-size control. Same syntax position, different meaning.

🔵 **ngspice-only statements:** `.DISTO`, `.NOISE` (also present in Xyce but
compare arg order), `.PZ`, `.SENS` (both dialects have a `.SENS`, but with
very different argument philosophy, see below), `.SP` (S-parameter, requires
RF-port `V` sources with `portnum`), `.PSS` (marked "experimental, not
publicly available" in the manual itself), `.FOUR freq ov1...` (batch-only
post-tran Fourier), `.PROBE` (auto-instrumentation, no Xyce equivalent),
`.WIDTH`. No native `.STEP` — parameter sweeps are emulated via a
`.control` loop using `alter`/`alterparam` + repeated `run`.

🟠 **Xyce-only statements:** `.STEP` (native outer parametric sweep —
one variable per line, multiple lines nest; sweep target may be a device's
"primary" param, `<device>:<param>`, `<model>:<param>`, a top-level
`.param`/`.global_param`, or `TEMP`), `.HB` (Harmonic Balance — RF/Sandia
addition, no ngspice equivalent), `.LIN` (S/Y/Z-parameter multiport
extraction via the `P` device, Touchstone output), `.SAMPLING`,
`.EMBEDDEDSAMPLING`, `.PCE` (statistical/UQ analyses replacing PSpice's
unsupported `.MC`/`.WCASE` — no ngspice equivalent found), `.DATA ... .ENDDATA`
(reusable sweep table referenced by `.AC`/`.DC`/`.NOISE`/`.STEP` via
`DATA=<name>`), `.RESULT`, `.SAVE`, `.PREPROCESS` (netlist-cleanup
directives: `REPLACEGROUND`/`REMOVEUNUSED`/`ADDRESISTORS`), `.FFT` (distinct
from `.FOUR`, its own window-function options).

**`.MPDE` — resolved, do not implement.** Checked directly: grepped the full
Xyce 7.10 Reference Guide, the full Xyce 7.10 Users' Guide (converted from
PDF), and the 7.10 Release Notes — zero occurrences of "MPDE" in any of
them. A web search confirms Multi-Time PDE analysis is a real Xyce/Sandia
research capability (cited in Sandia OSTI publications alongside HB, MOR,
and UQ as "active areas of research... capabilities are improving with each
release"), but it has **no documented, stable public netlist statement** in
the 7.10 release's own reference material. Conclusion: `.MPDE` is not part
of the public Xyce 7.10 netlist grammar — don't add it to the device/analysis
tables or offer it in completions; revisit only if a later Xyce version's
docs document a concrete `.MPDE` syntax.

🟡 **`.SENS` diverges sharply:** ngspice's `.SENS outvar [filters] [DC|AC ...]`
computes DC/AC sensitivity of one output to swept circuit elements pretty
broadly. Xyce's `.SENS objfunc=<expr> param=<param-list> [objvars=][acobjfunc=]`
requires the user to **explicitly enumerate target parameters** — Xyce will
not auto-sweep every circuit parameter — and needs a companion
`.options sensitivity direct=1 adjoint=1` line; AC-mode sensitivity needs
`objvars=`/`acobjfunc=` instead of `objfunc=`.

**`.MEASURE`/`.MEAS`** — 🟡 both have a *large* sub-grammar (TRIG-TARG,
FIND-WHEN, AVG/MIN/MAX/RMS/PP windowed stats, `param='expr'` forms) that is
similar in spirit between the two but not verified field-for-field
identical; Xyce's is documented as noticeably larger (continuous-mode
variants `AC_CONT`/`TRAN_CONT`/etc., a "Legacy Trig-Targ Mode" flag,
HSPICE-compatibility notes). Treat `.meas` as "common concept, dialect-specific
grammar" rather than assuming line-for-line portability.

**`.PRINT`/`.PLOT`** — 🟢 common concept (batch-mode tabular/plot output of
named vectors), 🟡 divergent option surface: Xyce's `.PRINT` has a much
richer `FORMAT=` list (`STD, NOINDEX, PROBE, TECPLOT, RAW, CSV, GNUPLOT,
SPLOT`) plus `FILTER=`/`DELIMITER=`/`TIMESCALEFACTOR=`; ngspice's `.PRINT`/`.PLOT`
distinction (tabular vs. ASCII-art plot) doesn't have a direct Xyce
equivalent — Xyce folds "plot-like" output into `FORMAT=` variants instead.

---

## 8. PSpice-Compatibility Layers (both dialects have one — worth tracking together)

Both ngspice and Xyce carry an explicit "translate PSpice-isms" compatibility
layer, which matters for an LSP that might ingest netlists written for a
third simulator:

- ngspice: `ngbehavior=ps/psa` mode (§12.11 of its manual) changes comment
  handling (`$` stops being a comment char), `.lib` fallback behavior, and
  more.
- Xyce: dedicated Reference-Guide chapter (§6.1) listing unsupported PSpice
  statements outright (`.PROBE, .VECTOR, .WATCH, .PLOT, .PZ, .DISTO, .TF,
  .AUTOCONVERGE, .MC, .WCASE, .DISTRIBUTION, .LOADBIAS, .SAVEBIAS,
  .ALIASES/.ENDALIASES, .STIMULUS, .TEXT`), plus semantic-flip warnings
  (`log()` meaning, `.STEP` model-parameter syntax, BSIM3 LEVEL numbering,
  `.TRAN UIC` handling, transmission-line current accessor names `I1()/I2()`
  vs PSpice's `IA()/IB()`), plus a step-by-step netlist-translation
  checklist, plus the **XDM** (Xyce Data Model) external translator tool for
  PSpice/HSPICE/Spectre → Xyce netlist conversion.

**LSP implication:** "which flavor of PSpice-compatibility quirks apply" is
effectively a third dialect axis on top of {ngspice, Xyce} × {native, various
compat modes} — worth deferring full support for until the core native
grammars are solid, but worth designing the dialect-selection mechanism to
leave room for it (e.g. `ngbehavior=ps` and Xyce's PSpice-translation notes
should map to the same conceptual "PSpice compatibility" diagnostic set where
possible).

---

## 9. Cross-Cutting Notes for LSP / Parser Design

1. **Dialect must be explicit, not sniffed.** Given the same-letter device
   collisions (`P`, `U`), the `^` operator meaning flip, and the `log()`
   meaning flip, a netlist cannot always be safely auto-detected as
   ngspice-vs-Xyce from content alone in the general case (though heuristics
   like `.HB`/`.STEP`/`.GLOBAL_PARAM` presence strongly suggest Xyce, and
   `.control`/XSPICE `A`-devices strongly suggest ngspice). Plan for a
   per-file or per-workspace dialect setting (e.g. a magic comment or
   extension-level config), with heuristic dialect *suggestion* as a
   fallback/lint, not silent assumption.
2. **Decision (adopted):** model the grammar as: core (🟢) + per-dialect
   device/statement tables + per-dialect expression-function tables — not as
   two independent grammars, and not as one grammar with if/else dialect
   checks scattered through it. The 🟢/🔵/🟠/🟡 tagging in this document is
   meant to map directly onto that architecture — a `commonGrammar` module
   plus `ngspiceExtensions`/`xyceExtensions` (and later `a reference toolExtensions`)
   overlay modules, each overlay supplying its own device table, operator
   table, and function table on top of the shared core. This is the
   structure the implementation (§ language discussion, TBD) should follow.
3. **Expression parsing needs a context tag**, at minimum: `{compile-time |
   behavioral/runtime | print/measure}` for ngspice's 3-4-way split, and
   `{param-context | print-context | device-param-context}` for Xyce's
   single-grammar-multiple-restriction-sets model (§4.4). This context also
   determines which diagnostics apply (e.g. flagging `V()` inside a `.param`
   line as illegal).
4. **High-value lint rules already visible from the manuals alone** (cheap
   wins once a basic parser exists): Xyce ternary `:`-collision (§4.3),
   Xyce `TABLE` PSpice-syntax normalization (§5.2), `^` operator meaning
   depends on declared dialect, `log()` meaning depends on declared dialect
   (+ `-hspice-ext math` flag if modeled), device letter meaning depends on
   declared dialect (`P`, `U`), `.options` flat-vs-packaged shape mismatch
   when "porting" a snippet between dialects.
5. **Gaps flagged during the initial extraction have since been resolved
   against primary sources** (Xyce Users' Guide, read directly via
   `pdftotext`, plus a web check for `.MPDE`):
   - Xyce's numeric-suffix table: confirmed (Users' Guide Table 4-1) —
     matches ngspice's table except Xyce adds `X` as a `Meg` alias and
     lacks `a` (atto) by default (§2).
   - Xyce's title-line requirement: confirmed mandatory, identical rule to
     ngspice (§1).
   - ngspice's `PARAMS:` keyword: confirmed **absent** — zero occurrences in
     the full manual; ngspice uses bare `ident=value`, do not accept
     `PARAMS:` in an ngspice-mode parser (§6.1).
   - Subcircuit name-mangling separator: confirmed **divergent** —
     ngspice uses `.` (dot), Xyce uses `:` (colon) by default (§6.1).
   - Circular subcircuit references: ngspice has no named "circular
     reference" error, but its purely-textual subckt expansion mechanism
     makes a cycle non-terminating by construction — treat as illegal,
     expect a hang/depth-limit failure rather than a clean diagnostic if
     unhandled (§6.1).
   - `.MPDE`: confirmed **not part of the public Xyce 7.10 netlist grammar**
     — absent from the Reference Guide, Users' Guide, and Release Notes;
     it's a real but research-grade/internal Sandia capability with no
     documented stable netlist syntax in this release (§7).

---

## 10. Quick common-vs-particular index

| Topic | Common? | Notes |
|---|---|---|
| Comment chars | 🟡 | `*` full-line same; EOL char differs (`$`/`//` vs `;`) |
| Line continuation | 🟢 | `+` prefix, same |
| Numbers/suffixes | 🟡 | same table (probably), Xyce's not explicitly documented |
| `M`=milli vs `Meg`=mega | 🟢 | shared classic-SPICE convention |
| Device letter table | 🟡 | mostly shared; `P`, `U` collide with different meanings |
| `.SUBCKT`/`.ENDS`/`X` | 🟢 | shared shape, minor keyword/mangling details unverified |
| `.MODEL` | 🟡 | shared shape; LEVEL numbering not portable; Xyce stricter on parens |
| `.PARAM` | 🟡 | shared shape; Xyce adds runtime-varying params + `.global_param` |
| Expression operators | 🟡 | `^` and `log()` meanings flip between dialects |
| Expression functions | 🟡 | large common core, each side has unique functions |
| B/E/G behavioral sources | 🟡 | shared concept and shape; table/interpolation and LAPLACE/FREQ support diverge |
| `.control` scripting | 🔵 | ngspice-only mechanism entirely; Xyce has no equivalent block |
| `.STEP` | 🟠 | native in Xyce; emulated via `.control` loop in ngspice |
| `.HB`/UQ analyses | 🟠 | Xyce/Sandia-specific. (`.MPDE` checked and confirmed **not** part of the public 7.10 netlist grammar — omit it.) |
| XSPICE code models | 🔵 | ngspice-only entirely |
| `.OPTIONS` shape | 🟡 | flat namespace (ngspice) vs. packaged namespaces (Xyce) |
| `.INCLUDE`/`.LIB` | 🟡 | include shared; `.lib` section semantics differ, Xyce targets HSPICE compat |
| PSpice-compat layer | 🟡 | both have one, different mechanisms and coverage |

---

## 11. a reference tool — TBD

a reference tool reference material has not been provided yet. Placeholder based on
general knowledge only — **treat everything in this section as unverified**
until real docs are supplied and this section is rewritten from them.

Known/likely facts to verify:
- a reference tool is a SPICE3-derived proprietary simulator (Analog Devices/Linear
  Technology), primarily driven through its own schematic format (`.asc`)
  but able to run/export plain-text `.net`/`.cir` netlists — the netlist
  syntax itself should be close to classic SPICE3/ngspice-native syntax.
  ngspice's own `ngbehavior=lt` compatibility mode (§2 above) is likely the
  best *proxy* for a reference tool's actual number-parsing and behavioral-source
  quirks until primary docs arrive — notably RKM notation (`2K7`, `4R7`) and
  specific `m`/`f`/`meg` disambiguation rules already captured there.
- a reference tool has a native `.step` directive (closer to Xyce's `.STEP` than to
  ngspice's emulate-via-`.control` approach) — needs verification of exact
  syntax.
- a reference tool ships its own component/model library and symbol format distinct
  from either ngspice's or Xyce's model libraries — out of scope for netlist
  *grammar* but relevant for a future "resolve model reference" LSP feature.
- Licensing note already discussed separately: parsing/tooling over
  a reference tool's plain-text netlist format (not its binary or proprietary model
  library content) is not expected to raise EULA concerns, but that was
  general orientation, not confirmed legal advice.

**Action item:** once a reference tool docs are added to the docs folder, repeat the
same extraction-agent process used for ngspice/Xyce and merge into this
section with the same 🟢/🔵/🟠/🟡-style tagging (adding a 🟣 a reference tool tag).

## 12. Project-specific extension: block/signal-domain statements

**Not part of any real SPICE dialect** — unlike every section above, this is this project's own
grammar extension, consumed by the `general-mna`/`general-simulator` sibling repos (a continuous-
block/signal-domain simulation layer built on top of `general-spice-core`'s parsed output). Kept
here rather than only in those repos' own docs specifically so this crate's grammar reference
stays the single source of truth for everything its own parser accepts.

**Syntax:** `NAME kind=<value> field=value field=value ...` — a bare identifier name, then any
number of whitespace-separated `key=value` fields (no leading device letter, no node list; a
field's value is a single token with no internal whitespace, including a Python-list-literal
value like `a=[[1,2],[3,4]]`).

**Dispatch rule:** distinguished from an ordinary element instance line purely by shape, not by
any reserved letter — no real SPICE element ever has a trailing field literally named `kind`
(elements use positional value/model-name arguments, never a `kind=` selector), so a line whose
second-or-later token starts with `kind=` is unambiguously a block statement instead. See
[`ast::BlockInstance`] and `parser::is_block_instance_line`/`parser::parse_block_instance`
(shared by both dialects) for the exact implementation.

**Semantics:** none, at this layer — same as this parser doesn't know what a resistor *does*,
it doesn't know what `kind=pid`/`kind=statespace`/etc. mean, only that the line has this shape.
Every field's value (including `kind` itself) is captured as raw, unparsed text; a downstream
builder (`general-mna`) is the one place that interprets `kind` and the rest of the fields.

**Nesting:** a block statement nests inside a `.subckt` scope exactly like any other non-
`.subckt`/`.ends` statement (see [`symbols::scope::build_scope_tree`]) — no special-casing was
needed for that to already work correctly.

**Backward compatibility:** the older convention some existing decks still use — a block/field
line written as `* NAME kind=...`, disguised as an ordinary full-line SPICE comment so this
parser would skip it entirely — continues to work exactly as before (still parses as
[`ast::Statement::Comment`]). This section documents the new, first-class alternative; it
doesn't require migrating anything.
