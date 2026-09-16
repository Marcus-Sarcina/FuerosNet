//! The RHTN acceptance suite.
//!
//! `acceptance.json` beside this crate is the authority: one entry per
//! acceptance test, each citing the specification sections it answers to and
//! quoting the sentence that justifies its expectation.  `tools/check.py`
//! verifies every citation and every quote against the specification text and
//! reports coverage of the gaps `Robot/implementation-plan.md` section 8 lists.
//! `tools/gen_stubs.py` writes `tests/*.rs` from the catalogue; those files are
//! generated and never edited by hand.
//!
//! A test is implemented in the crate that owns the behaviour, marked with a
//! comment `// acceptance: XXX-NN` on the test.  The generator omits the stub
//! for any id it finds implemented, so the ignored count here is the count of
//! tests still owed.
