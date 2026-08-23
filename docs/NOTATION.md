# MuseCode notation

The textual form of a `Music` tree. `impl Display for Music` prints it; the M2 parser (#14) must accept exactly what `Display` prints, so this file is the contract between the two. Whitespace, including line breaks, carries no meaning: every run of whitespace is one separator.

## Grammar

```
piece     := element (ws element)*
element   := note | rest | chord | group | par | modify

note      := pitch ':' dur attrs
rest      := 'r:' dur
chord     := '[' pitch (ws pitch)* ']:' dur attrs      all notes share dur and attrs
group     := '(' piece ')'                             a Seq nested inside a Seq
par       := '{' piece ('|' piece)* '}'                Par; an empty Par is '{  }'
modify    := ctrl ws '{' piece '}'

pitch     := chromatic | degree | interval
chromatic := letter accidental? octave                 C4  Eb5  F#3  Cb5  Abb4  B-2
letter    := 'A'..'G'
accidental:= 'bb' | 'b' | '#' | '##'
octave    := '-'? digit+                               scientific pitch notation, C4 = MIDI 60
degree    := ('b'+ | '#'+)? number ("'"* | ','*)       1  b3  #7  bb3  5'  1,,
                                                       one ' per octave up, one , per octave down
interval  := ('+' | '-') quality number                +M3  -P5  +m2  +d5  +A4  +dd7  -AA4
quality   := 'dd' | 'd' | 'm' | 'P' | 'M' | 'A' | 'AA'

dur       := ('w'|'h'|'q'|'e'|'s'|'t') ('.' | '..' | '3')?   q  e.  h..  q3
           | num '/' den                                      5/4  9/8  0/1
             w h q e s t = whole half quarter eighth sixteenth thirty-second
             '.' dotted, '..' double-dotted, '3' the triplet of the value
             anything else prints as a reduced fraction of a quarter note

attrs     := artic? tie? velocity? voice? hint*
artic     := '-.' | '-!' | '--' | '->' | '-^' | '-' name
             staccato staccatissimo tenuto accent marcato; every other articulation by name:
             -legato -slur -fermata -pizzicato -arco -trill -mordent -turn
             -harmonic_natural -harmonic_artificial
tie       := '~'                                       tied to the next note of the same pitch
velocity  := '@v' number                               1..=127
voice     := '@voice(' name ')'
hint      := '@hint(' debug ')'                        the Rust Debug form of a BackendHint

ctrl      := 'key(' pitchclass ws mode ')'             key(F minor)  key(C lydian)
           | 'scale(' (string ws)? number (ws number)* ')'   scale(0 2 4 7 9)  scale("major" 0 2 4 5 7 9 11)
           | 'tempo(' bpm ')' | 'tempo(' bpm '->' bpm ':' beats ')'   tempo(96)  tempo(96->120:8)
           | 'time(' num '/' den ')'                   time(4/4)
           | 'transpose(' int ')'                      transpose(7)
           | 'transpose_diatonic(' int ')'             transpose_diatonic(-1)
           | 'dyn(' level ')'                          dyn(mf)  dyn(<)  dyn(>)
           | 'art(' name ')'                           art(staccato)
           | 'voice(' name ')'                         voice(left)
           | 'instrument(' name ')'                    instrument(bandoneon)
           | 'hint(' debug ')'                         hint(Midi(Channel(3)))
           | 'user(' string ws string ')'              user("k" "v")
mode      := 'major' | 'minor' | 'dorian' | 'phrygian' | 'lydian' | 'mixolydian'
           | 'locrian' | 'harmonic_minor' | 'melodic_minor' | 'custom(' number ')'
level     := 'ppp' | 'pp' | 'p' | 'mp' | 'mf' | 'f' | 'ff' | 'fff' | 'sfz' | 'fp' | '<' | '>'
```

## Layout rules

`Display` chooses line breaks so that a piece reads one bar per line; a parser ignores them.

1. Notes, rests and chords are **leaves**. A `Seq` whose children are all leaves prints on one line, separated by single spaces.
2. A `Par` prints inline as `{ a | b | c }` when every child prints inline. A `Par` of notes that share one duration and identical attributes prints as a chord instead.
3. A `Modify` prints inline as `ctrl { piece }` when its body prints inline. Nested modifiers chain on one line: `tempo(96) { time(4/4) { key(F minor) { ... } } }`, outermost first.
4. A `Seq` with at least one compound child (`Par`, `Modify`, nested `Seq`) prints one child per line; runs of consecutive leaves share a line. A nested `Seq` is parenthesised as a group.
5. When a `Par` or `Modify` cannot print inline it opens a block: the `Par` prints `{`, each child indented by two spaces, `|` on its own line between children, and `}`; the `Modify` prints its chained heads on one line, the body indented, and the matching closers `} } }` on one line.

## Examples

A note with every attribute:

```
G3:q.-.~@v100@voice(bass)@hint(Midi(Channel(2)))
```

dotted quarter G3, staccato, tied to the next note, velocity 100, in voice `bass`, with a MIDI channel hint.

The README Piazzolla sketch prints as:

```
tempo(96) { time(4/4) { key(F minor) {
  { G3:e. G3:s G3:e G3:e G3:q G3:q | r:q r:q. | 5:q b5:e 4:e 3:h }
  { C3:e. C3:s C3:e C3:e C3:q C3:q | r:q r:q. | transpose_diatonic(-1) { 5:q b5:e 4:e 3:h } }
  { F3:e. F3:s F3:e F3:e F3:q F3:q | r:q r:q. | 1:w }
  { F3:e. F3:s F3:e F3:e F3:q F3:q | r:q r:q. | 1:w }
} } }
```

A `Par` that cannot print inline, wrapped in two modifiers:

```
tempo(60) { key(C major) {
  {
    C4:q
    { D4:h | E4:q }
  |
    G3:w
  }
} }
```

## What the notation does not carry

- The `name` of a `Scale` is printed when present but is display-only; two scales that differ only by name are the same scale.
- Hints and `user(...)` values print in Rust `Debug` form. A parser should accept that form; M2 may give the common hints a shorter spelling.
- Nothing here resolves pitch. `b3` prints as written; what it sounds like depends on the `key` in scope (see the `resolve` module).
