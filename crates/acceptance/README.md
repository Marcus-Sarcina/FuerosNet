# Acceptance catalogue

**`acceptance.json` is the authority.**  One entry per acceptance test, covering
the gap rows of `Robot/implementation-plan.md` section 8.2: the behaviour of the
running system that no formal model and no test vector establishes.  What the
models and vectors do establish (section 8.1 of the plan) is not restated here.

## An entry

```
id             XXX-NN; the prefix fixes the area, the area fixes the gap row
title          one imperative sentence
milestone      1..5, "after-5", or "manual"
kind           positive | negative | must-accept | liveness | robustness | manual
spec           qualified citations only: design §N, wire-format.md §N,
               light-client-requirements.md §N, infra-client-requirements.md §N,
               resource-requirements.md §N, design Appendix A.N
rule           the sentence that justifies the expectation, quoted verbatim from
               a cited section, at most 40 words
given / when / then
               the state, the stimulus, the observation; `then` is observable
               from outside the component and never names an implementation
oracle         fixture | model | behaviour
interpretation null, or the one assumption the specification does not state
```

**A `then` that cannot quote its sentence is an interpretation and says so.**
That field is the list of places where a test encodes a reading of the
specification rather than its text; each is a review target, as the test-vector
README treats its own interpretations.

## The tools

- `tools/check.py` — every field, id, citation and quote checked against the
  specification files; every gap row covered; numbers reported, not states.
  No exemptions.
- `tools/gen_stubs.py` — writes `tests/<area>.rs`, one `#[ignore]` stub per entry
  still owed.  Generated; regenerate rather than edit.

## Implementing an entry

Write the real test in the crate that owns the behaviour and put
`// acceptance: XXX-NN` on it.  The generator then omits the stub and the checker
counts the id as implemented.  The catalogue entry stays as written; if
implementation shows the entry wrong, the entry changes, with the specification
if the specification was wrong, and never the marker alone.

## Unset values

Where design §21.1 leaves a value unset, an entry parameterises it and says so in
`given`.  The suite does not choose parameter values; the author does.
