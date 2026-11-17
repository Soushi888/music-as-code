---
project: musecode
type: architecture
status: exploration
created: 2026-11-17
---

# Semantic music: exploration notes and handoff

Handoff from a design conversation. Nothing here is implemented. It records what the codebase already offers, what is missing, and a proposed order of work, so a new session can pick up without the conversation. Claims about the code come from reading `README.md`, `docs/ARCHITECTURE.md`, `docs/NOTATION.md`, `attrs.rs`, `control.rs`, `music.rs` (`chord`), `phrase.rs` (hashing) and the public signatures of `rhythm`, `theory`, `analysis`. `resolve.rs` was not read in full and the test suite was not run.

"Semantic music" is taken here to mean a representation that carries meaning (harmonic function, voice role, rhythmic gesture, relations between fragments), not only sounding events.

## 1. What is already semantic

| Existing choice | Why it counts |
|---|---|
| `Pitch = Chromatic \| Degree \| Interval` | `d!(b 3)` means "minor third in the current key", resolved late. Changing `Key` moves the melody without rewriting notes. |
| `Modify(Control, _)` | Context scoped to a subtree, like lexical scope. |
| `rhythm::Pattern` and `.on(pitch)` | A named rhythmic gesture with no pitch; structure separated from realisation. |
| `euclid` and its test (E(5,8) legato is the cinquillo; the habanera is not Euclidean) | A musicological claim, stated and checked. |
| `resolve` | The single pass that gives `Degree`, `Interval` and `Control` meaning. |
| `analysis` (`label_triad`, `vertical_slices`) | Recovers meaning from events. |
| `Phrase` plus Blake3 | Stable identity for a fragment. |
| `docs/NOTATION.md` | A formal grammar that round-trips with the tree (contract with the M2 parser). |
| Principle 5 (hints beside the IR) | What the music is, apart from how a backend renders it. |

## 2. Gaps

1. **Intent is erased at `resolve`.** `Resolved` is a flat event list. An `Event` does not know its source node, so analysis cannot point back into the tree.
2. **`Chord`, `Key`, `Voicing` are data with no link to `Music`.** `theory.rs` has no resolution logic and a `Chord` is not a tree node. The ii-V-I of the tango exists only as notes.
3. **Hash identity is structural.** Two trees that mean the same music hash differently (see section 4).
4. **Roles are strings.** `VoiceId(String)` and `Control::User(String, String)` are untyped escape hatches. Open question 3 (`Score`, voices) is really a question about roles.
5. **No motif or phrase semantics.** Nothing records that a fragment is a transposition, retrograde or variation of another.

## 3. Layer proposal: annotations beside the IR

Do not add a sixth constructor: it would break design principle 5 and the five-constructor argument in `ARCHITECTURE.md`. Add a side table keyed by fragment hash, like hints.

```rust
// new module `semantics`, above resolve; imports no backend and not phrase
pub enum Fact {
    Harmony(HarmonicFunction),      // Tonic, Predominant, Dominant, or ii / V / I
    Role(Role),                     // Melody, Bass, Comping, CounterMelody
    Gesture(GestureId),             // Tresillo, Euclid { hits: 3, steps: 8 }, Habanera
    Relation(Relation, PhraseHash), // TransposeOf, RetrogradeOf, VariationOf
    Cadence(CadenceKind),
}
pub struct Annotations(BTreeMap<blake3::Hash, Vec<Fact>>);
```

Facts can be written by hand, inferred by `analysis` (`label_triad` already yields harmony candidates), or proposed by a language model and checked. A fact attached to a hash follows the fragment when it moves, and stops applying when the fragment changes.

Prerequisite: give each `Event` a `NodeId` (path or hash of the source node) so the chain note, node, facts works in both directions.

## 4. Identity: the hash must be layered

**Correction to an earlier claim in the conversation.** The README says `explicit`, `by_degree` and `by_chord` are "three ways to write the same C major triad". They are not. `chord([C4,E4,G4], h())` builds a `Par` of three notes with one duration (a block chord). `seq![C4 q, E4 q, G4 h]` is a `Seq` (an arpeggio). Their different hashes are correct. The README comment is inaccurate and should be fixed.

The real equivalence case: `n(C4)` equals `n(d!(1))` only under `Key` C. A degree fragment is a function of its context, not a value. Semantic identity is therefore relative to context.

Proposed hashes, each answering a different question:

| Hash | Computed over | Question |
|---|---|---|
| Structural (exists) | the tree as written | same source exactly? |
| Canonical | the tree after rewriting to normal form | same expression up to writing details? |
| Resolved | `(fragment, context)` to events | same sound in this context? |

Syntactic equivalences that `canonicalize` can apply (several already appear in `tests/laws.rs`):

- `Par` is commutative: `Par([a,b])` and `Par([b,a])` are the same music but hash differently today because child order is serialised. Sort children by hash.
- `Seq([x])` is `x`; nested `Seq` flattens (the `+` operator flattens but the `seq!` macro does not); adjacent `Rest(a)`, `Rest(b)` merge to `Rest(a+b)`.
- `Modify(Transpose(0), x)` is `x`; nested `Transpose(a)`, `Transpose(b)` merge to `Transpose(a+b)`.
- `retrograde` twice is identity; `augment(h).diminish(h)` is identity.

If the rules are not confluent, use an e-graph (`egg`). The same rule engine can later host concept rules (section 5). Model: Unison hashes the normalised AST so names do not affect identity.

### Hash stability risk (to confirm)

`Phrase` hashes `bincode::serialize(&music)` with Blake3. Enum variant order is part of the encoding, so inserting a variant in `Control` or `Articulation` changes every hash. There is no schema version tag or domain separation, and a `bincode` upgrade could change bytes. Only `phrase.rs` was read for this, not its tests, so treat it as a risk to verify. If hashes become shared or persisted, specify a canonical encoding (in the spirit of IPLD DAG-CBOR) with a version tag before they are in circulation.

## 5. Musical ontology engine (longer horizon)

An AST makes a musical concept definable as a pattern over the tree. Three levels:

| Level | Queried | Example |
|---|---|---|
| Syntactic | shape of the tree | a `Par` of two voices offset by one beat with the same note sequence (canon) |
| Semantic | resolved events | dominant seventh; onsets 0, 3/2, 3 in 4 beats (tresillo) |
| Ontological | relations between concepts | tresillo is subsumed by Euclidean E(3,8); the habanera is not |

Key idea: a concept is both a constructor and a recogniser.

```
concept Tresillo       : Pattern = onsets [0, 3/2, 3] over 4 beats
concept DominantSeventh(r)        = chord { r, +M3, +P5, +m7 }
concept AuthenticCadence(k)       = Dominant(k) ; Tonic(k)     // adjacency in a Seq
subsumes  Euclid(3,8) >= Tresillo
```

`DominantSeventh(G3)` generates; the same definition finds every dominant seventh in a piece and its root; subsumption lets you ask "does this piece contain a Euclidean rhythm" without anyone having written that word.

Pipeline:

```
Music --extract--> facts --theory rules--> derived facts --> Annotations --> interface
 (IR)   note(n,p,onset,dur), seq_child(p,i,c), in_key(n,k)
```

Design points:

- Rule engine: Datalog (`ascent` or `datafrog`), not a full OWL reasoner.
- Plural theories (tango, tonal jazz, Toussaint rhythms) in separate namespaces, since musical concepts are culturally contested; keep each derived fact's originating theory, and a confidence level when inferred.
- A separate crate (for example `musecode_onto`) depending on `musecode_core`, never the reverse, so the core stays a minimal IR.
- Concepts with holes need a separate `MusicPattern` type with wildcards. Do not add variables to `Music`.
- Cache derived facts per fragment hash, as `RenderCache` already does for renders.

Risk: scope explosion. Prototype narrowly first: tresillo, habanera, cinquillo, triads and sevenths, V-I cadence, and the Euclidean subsumption already tested.

## 6. Interface sketch

The tree, its textual notation and the resolver are the layers of a projectional editor:

1. **Text view**: `Display` and the M2 parser, with semantic comments, for example `key(F minor) { ii V I }`.
2. **Outline view**: the tree of facts (intro, ii-V-I, cadence); selecting "V" highlights its notes through `NodeId`.
3. **Lens view** (piano roll or score): the same events coloured by role or function.
4. **Semantic verbs** rather than note edits: "same rhythm, other harmony" (keep `Gesture`, change `Harmony`); "go minor" (`Control::Key`, thanks to degrees); queries such as "all tresillos" or "all dominants".
5. **Natural-language input** compiled to a combinator pipeline that `analysis` and the law tests can verify. The typed tree stays the source of truth.

## 7. Proposed order of work

1. Fix the README comment about the "three ways" triad (section 4).
2. `canonicalize(&Music) -> Music` with five or six rules, plus property tests in `tests/laws.rs`: idempotent, and two terms related by a law share a canonical hash.
3. Decide the hash encoding (versioned canonical encoding) before hashes circulate.
4. Add `NodeId` to `Event` in `resolve`.
5. Add module `semantics` (`Fact`, `Annotations`). Test: annotate the tango ii-V-I and recover each bar's function with `label_triad`.
6. Make `Chord` a first-class tree citizen (for example `chord_symbol(Chord, Voicing) -> Music` carrying a `Harmony` fact); this meets M3 voicings.
7. Prototype the ontology engine in a separate crate with the narrow concept set from section 5.

## 8. Open questions

- Which sense of "semantic" is the priority: musical analysis (function, role, motif) or knowledge-graph ontologies (Music Ontology, MEI, linked data)? The annotation layer serves both; the order of work differs.
- Whether canonical equivalence should treat `Degree` and `Chromatic` as equal only under a stated `Key` (the proposed resolved hash) or also offer a normal form that always prefers degrees.
- Whether decentralised sharing of content-addressed fragments is a goal. Nothing in the current code requires it.
