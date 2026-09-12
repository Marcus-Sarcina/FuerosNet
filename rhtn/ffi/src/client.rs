//! What a shell asks of the client.
//!
//! `rhtn-adaptors` already binds a client to a node and a socket and runs
//! it on a thread of its own. This is that handle with the awaiting taken
//! off: a shell calls, and is called back.
//!
//! What it owes:
//!
//! - **The ceremony, step by step** (`light-client-requirements.md` §1):
//!   begin, the proximity channels, capture, the verifiers selected and
//!   queried, the witnesses asked, the proposal reviewed and signed. Each
//!   step is a call and each abort is a value, because a screen has to name
//!   what failed.
//! - **Consent and the key grant** (§1.4, §1.5), which the subject alone
//!   answers and which the person must be shown before it is signed.
//! - **Attach, maintain and payload** (§3, §4), and the direct path offered
//!   or declined per peer (§5).
//! - **Recovery** (§2): the meeting, the block, the adoption, and the
//!   lines sealed under the prior key.
//! - **Notices** (design Appendix A.3): what the person is told rather than
//!   asked, delivered as they occur and not polled for.
//!
//! What it must not do: hold state the client holds. A shell that keeps its
//! own copy of the ceremony's progress will disagree with the client about
//! what was signed, and the client is the one whose bytes are on the wire.
