//! The ceremony's conversation, against the canonical vectors
//! (`wire-format.md` §7.10.1 kinds 9 to 18, §7.10.2).
//!
//! **The point of this file is byte equality with the corpus**, as it is
//! for the local interfaces: the corpus is generated from the specification
//! by a tool that shares no code with this crate, so a mismatch means one
//! of the two is wrong and neither gets to decide which.  The second half
//! is the in-process message each structure stands for, across the two
//! functions that map between them.

mod common;

use common::fixture;
use rhtn_client::ceremony::{Msg, Proposed, conversation_from, conversation_payload};
use rhtn_client::conversation::*;
use rhtn_client::device::{ChannelKind, ChannelResult};
use rhtn_client::payload::*;
use rhtn_client::query::VerificationQuery;
use rhtn_client::record::{LABELS, Refusal, disclosure_root};
use rhtn_codec::cbor::{Item, parse_all};

fn last32(b: &[u8]) -> [u8; 32] {
    b[b.len() - 32..].try_into().unwrap()
}

// acceptance: DEC-36
#[test]
fn the_conversation_objects_encode_to_the_canonical_bytes_and_read_back() {
    let fx = fixture("P-consent-reply");
    let read = ConsentReply::decode(&fx).expect("the canonical reply decodes");
    assert_eq!(read.query_id, fx[3..35], "the query id, field 1");
    assert!(
        matches!(parse_all(&read.consent), Ok(Item::Array(ref a)) if a.len() == 4),
        "the consent is the COSE_Sign1 as it was signed"
    );
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-fishing-proposal");
    let read = FishingProposal::decode(&fx).expect("decodes");
    assert_eq!(read.entries.len(), 1, "one entry, the envelope form");
    assert!(matches!(parse_all(&read.entries[0]), Ok(Item::Map(_))));
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-witness-request");
    let read = rhtn_client::ceremony::WitnessRequest::decode(&fx).expect("decodes");
    // the participants as the fixture lists them, alice then bob
    assert_eq!(read.participants[0], fx[40..72], "alice's keyhash");
    assert_eq!(read.participants[1], fx[74..106], "bob's keyhash");
    assert_eq!(
        read.channels
            .iter()
            .map(|c| (c.kind, c.result, c.resolution_m))
            .collect::<Vec<_>>(),
        vec![
            (ChannelKind::Nfc, ChannelResult::Pass, None),
            (ChannelKind::Optical, ChannelResult::Pass, None),
        ],
        "NFC and optical passing"
    );
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-witness-answer-witnessing");
    let read = WitnessAnswer::decode(&fx).expect("decodes");
    assert_eq!(read.bits, Some(7), "protocol ran, both responsive, latency");
    assert_eq!(read.encode(), fx);
    let fx = fixture("P-witness-answer-declining");
    let read = WitnessAnswer::decode(&fx).expect("decodes");
    assert_eq!(read.bits, None, "declining carries no bits");
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-gathered-responses");
    let read = GatheredResponses::decode(&fx).expect("decodes");
    assert_eq!(read.responses.len(), 3, "the normal record's three");
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-back-pointers");
    let read = BackPointers::decode(&fx).expect("decodes");
    assert_eq!(read.txids.len(), 2);
    assert_eq!(read.encode(), fx);
    let fx = fixture("B-back-pointers-8");
    let read = BackPointers::decode(&fx).expect("eight, the ceiling, decodes");
    assert_eq!(read.txids.len(), 8);
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-proposed-body");
    let read = ProposedBody::decode(&fx).expect("decodes");
    assert_eq!(
        read.set.iter().map(|d| d.label).collect::<Vec<_>>(),
        LABELS,
        "seven slots in label order"
    );
    let (proposal, back) = proposal_from_body(&read.body).expect("the body reads back");
    assert_eq!(
        proposal.root,
        disclosure_root(&read.set),
        "the root is the root of the set shown"
    );
    assert_eq!(proposal.witnesses.len(), 16);
    assert_eq!(proposal.responses.len(), 3);
    assert_eq!(back.len(), 18, "one list per signer");
    assert_eq!(
        proposal.body(&back),
        read.body,
        "and re-emits byte for byte"
    );
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-signing-reply-signed");
    let read = SigningReply::decode(&fx).expect("decodes");
    let entries = read.reply.clone().expect("the signer signed");
    // two entries, as `sign_entries` produces them: wrapped for the wire
    // and unwrapped here
    let mut wrapped = vec![0x82];
    wrapped.extend_from_slice(&entries);
    assert!(
        matches!(parse_all(&wrapped), Ok(Item::Array(ref a)) if a.len() == 2),
        "two consecutive COSE_Signature items"
    );
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-signing-reply-refused");
    let read = SigningReply::decode(&fx).expect("decodes");
    assert_eq!(
        read.reply,
        Err(Refusal::NomineeNotMine(last32(&fx))),
        "refusal 1 names the witness"
    );
    assert_eq!(read.encode(), fx);

    let fx = fixture("P-signing-reply-refused-4");
    let read = SigningReply::decode(&fx).expect("decodes");
    assert_eq!(
        read.reply,
        Err(Refusal::NotVerified),
        "refusal 4 names nothing"
    );
    assert_eq!(read.encode(), fx);
}

// acceptance: DEC-36
#[test]
fn an_object_the_schema_refuses_does_not_decode_here_either() {
    assert!(
        BackPointers::decode(&fixture("B-back-pointers-9")).is_err(),
        "nine back-pointers"
    );
    assert!(
        BackPointers::decode(&fixture("N-back-pointers-empty")).is_err(),
        "a signer sends at least one"
    );
    assert!(
        WitnessAnswer::decode(&fixture("N-witness-answer-bits-while-declining")).is_err(),
        "bits while declining"
    );
    assert!(
        GatheredResponses::decode(&fixture("N-gathered-responses-33")).is_err(),
        "thirty-three responses"
    );
    assert!(
        ProposedBody::decode(&fixture("N-proposed-body-six-slots")).is_err(),
        "six slots"
    );
    assert!(
        SigningReply::decode(&fixture("N-signing-reply-both")).is_err(),
        "entries and a refusal"
    );
    assert!(
        SigningReply::decode(&fixture("N-signing-reply-particular-alone")).is_err(),
        "a particular beside the entries"
    );
    assert!(
        SigningReply::decode(&fixture("N-signing-reply-code-5")).is_err(),
        "refusal codes run 1 to 4"
    );
    // and the kinds are not interchangeable, each checking against its own
    // rule
    assert!(ConsentReply::decode(&fixture("P-back-pointers")).is_err());
    assert!(BackPointers::decode(&fixture("P-consent-reply")).is_err());
    assert!(WitnessAnswer::decode(&fixture("P-signing-reply-refused")).is_err());
    assert!(SigningReply::decode(&fixture("P-witness-answer-witnessing")).is_err());
    assert!(ProposedBody::decode(&fixture("P-witness-request")).is_err());
    assert!(rhtn_client::ceremony::WitnessRequest::decode(&fixture("P-proposed-body")).is_err());
    // the same bytes under the wrong kind are refused on the way in
    assert!(conversation_from(KIND_BACK_POINTERS, &fixture("P-consent-reply")).is_err());
    assert!(conversation_from(KIND_RECORD, &fixture("P-proposed-body")).is_err());
}

// acceptance: DEC-36
#[test]
fn a_message_crosses_as_its_kind_and_comes_back_the_same() {
    // each in-process message, to its kind and the corpus bytes, and back
    let cases: Vec<(&str, u64, Msg)> = vec![
        (
            "P-verification-query",
            KIND_CONSENT_REQUEST,
            Msg::ConsentRequest(
                VerificationQuery::decode(&fixture("P-verification-query")).unwrap(),
            ),
        ),
        ("P-consent-reply", KIND_CONSENT_REPLY, {
            let r = ConsentReply::decode(&fixture("P-consent-reply")).unwrap();
            Msg::Consent {
                query_id: r.query_id,
                consent: r.consent,
            }
        }),
        (
            "P-witness-request",
            KIND_WITNESS_REQUEST,
            Msg::WitnessRequest(
                rhtn_client::ceremony::WitnessRequest::decode(&fixture("P-witness-request"))
                    .unwrap(),
            ),
        ),
        (
            "P-witness-answer-witnessing",
            KIND_WITNESS_ANSWER,
            Msg::WitnessAnswer(Some(7)),
        ),
        (
            "P-witness-answer-declining",
            KIND_WITNESS_ANSWER,
            Msg::WitnessAnswer(None),
        ),
        (
            "P-gathered-responses",
            KIND_GATHERED_RESPONSES,
            Msg::Responses(
                GatheredResponses::decode(&fixture("P-gathered-responses"))
                    .unwrap()
                    .responses,
            ),
        ),
        (
            "P-back-pointers",
            KIND_BACK_POINTERS,
            Msg::BackPointers(
                BackPointers::decode(&fixture("P-back-pointers"))
                    .unwrap()
                    .txids,
            ),
        ),
        ("P-proposed-body", KIND_PROPOSED_BODY, {
            let pb = ProposedBody::decode(&fixture("P-proposed-body")).unwrap();
            let (proposal, back) = proposal_from_body(&pb.body).unwrap();
            Msg::Proposal(Box::new(Proposed {
                proposal,
                set: pb.set,
                back,
            }))
        }),
        (
            "P-signing-reply-signed",
            KIND_SIGNING_REPLY,
            Msg::Signed(
                SigningReply::decode(&fixture("P-signing-reply-signed"))
                    .unwrap()
                    .reply,
            ),
        ),
        (
            "P-signing-reply-refused",
            KIND_SIGNING_REPLY,
            Msg::Signed(Err(Refusal::NomineeNotMine(last32(&fixture(
                "P-signing-reply-refused",
            ))))),
        ),
        (
            "P-signing-reply-refused-4",
            KIND_SIGNING_REPLY,
            Msg::Signed(Err(Refusal::NotVerified)),
        ),
        (
            "P-normal-record",
            KIND_RECORD,
            Msg::Record(fixture("P-normal-record")),
        ),
    ];
    for (id, kind, msg) in cases {
        let (k, bytes) = conversation_payload(&msg).unwrap_or_else(|| panic!("{id} is carried"));
        assert_eq!(k, kind, "{id}: its kind");
        assert_eq!(bytes, fixture(id), "{id}: the corpus bytes");
        assert_eq!(
            conversation_from(k, &bytes).unwrap_or_else(|e| panic!("{id} reads back: {e:?}")),
            msg,
            "{id}: the same message back"
        );
    }

    // the refusals without a corpus fixture: an omitted response names the
    // query id; a clock refusal carries nothing, so what the witness
    // observed stays with it and the particulars read back as zero
    let omitted = Msg::Signed(Err(Refusal::OmittedResponse([7u8; 32])));
    let (k, bytes) = conversation_payload(&omitted).unwrap();
    assert_eq!(conversation_from(k, &bytes).unwrap(), omitted);
    let (k, bytes) = conversation_payload(&Msg::Signed(Err(Refusal::ClockFar {
        claimed: 1_782_781_200,
        observed: 1_782_000_000,
    })))
    .unwrap();
    assert_eq!(bytes, [0xa1, 0x02, 0x03], "refusal 3 and nothing beside it");
    assert_eq!(
        conversation_from(k, &bytes).unwrap(),
        Msg::Signed(Err(Refusal::ClockFar {
            claimed: 0,
            observed: 0
        }))
    );

    // what is not the conversation's is None, not a kind
    assert_eq!(conversation_payload(&Msg::PoolExhausted), None);
    assert_eq!(
        conversation_payload(&Msg::Grant(vec![1, 2, 3])),
        None,
        "the grant is the verifier path's (kind 1), not the conversation's"
    );
    // a fishing proposal decodes and has no message yet
    assert!(FishingProposal::decode(&fixture("P-fishing-proposal")).is_ok());
    assert!(conversation_from(KIND_FISHING_PROPOSAL, &fixture("P-fishing-proposal")).is_err());
    assert!(conversation_from(KIND_APPLICATION, b"hello").is_err());
}

/// A proposed body is checked as every archive will check it, before
/// anything is read out of it: a body this client would re-emit the same
/// way is still refused where the archives would refuse it.
#[test]
fn a_proposed_body_the_archives_would_refuse_is_refused_before_it_is_read() {
    let fx = fixture("P-proposed-body");
    let read = ProposedBody::decode(&fx).expect("decodes");
    let (proposal, back) = proposal_from_body(&read.body).expect("the body reads back");
    // the same proposal over a merge list that repeats a txid re-emits to
    // the same bytes it was built from, and is a body no archive takes:
    // a merge list is sorted and repeats nothing (§3.1).  An unsorted
    // list would not do here, since the emitter sorts what it is given
    let mut repeated = back.clone();
    repeated[0] = vec![[0x11u8; 32], [0x11u8; 32]];
    let body = proposal.body(&repeated);
    assert!(
        proposal_from_body(&body).is_err(),
        "a repeated back-pointer is refused as the archives refuse it"
    );
    // and the codec refuses the body inside the ProposedBody itself
    let shown = ProposedBody {
        body: vec![0xa0],
        set: read.set.clone(),
    }
    .encode();
    assert!(
        ProposedBody::decode(&shown).is_err(),
        "an empty map is a byte string and no presence body"
    );
    let shown = ProposedBody {
        body,
        set: read.set,
    }
    .encode();
    assert!(ProposedBody::decode(&shown).is_err());
}

/// A refusal's particular is exactly 32 bytes (`wire-format.md` §7.10.2):
/// a witness keyhash or a query id, and nothing of another width.
#[test]
fn a_signing_reply_particular_of_another_width_is_refused() {
    let mut short = vec![0xa2, 0x02, 0x01, 0x03, 0x58, 0x1f];
    short.extend_from_slice(&[9u8; 31]);
    assert!(SigningReply::decode(&short).is_err(), "31 bytes");
    let mut long = vec![0xa2, 0x02, 0x01, 0x03, 0x58, 0x21];
    long.extend_from_slice(&[9u8; 33]);
    assert!(SigningReply::decode(&long).is_err(), "33 bytes");
    let mut exact = vec![0xa2, 0x02, 0x01, 0x03, 0x58, 0x20];
    exact.extend_from_slice(&[9u8; 32]);
    assert!(
        matches!(
            SigningReply::decode(&exact).map(|r| r.reply),
            Ok(Err(Refusal::NomineeNotMine(k))) if k == [9u8; 32]
        ),
        "32 bytes, the witness keyhash"
    );
}

/// A refusal names its particular where it has one and nowhere else
/// (`wire-format.md` §7.10.2): 1 and 2 carry it, 3 and 4 do not, and the
/// codes run 1 to 4.
#[test]
fn a_refusal_carries_its_particular_only_where_it_names_one() {
    assert!(
        SigningReply::decode(&[0xa1, 0x02, 0x01]).is_err(),
        "refusal 1 without its witness"
    );
    assert!(
        SigningReply::decode(&[0xa1, 0x02, 0x02]).is_err(),
        "refusal 2 without its query"
    );
    let mut with = vec![0xa2, 0x02, 0x04, 0x03, 0x58, 0x20];
    with.extend_from_slice(&[9u8; 32]);
    assert!(
        SigningReply::decode(&with).is_err(),
        "refusal 4 with a particular"
    );
    assert_eq!(
        SigningReply::decode(&[0xa1, 0x02, 0x04]).map(|r| r.reply),
        Ok(Err(Refusal::NotVerified)),
        "refusal 4 and nothing beside it"
    );
    assert!(
        SigningReply::decode(&[0xa1, 0x02, 0x05]).is_err(),
        "no refusal 5"
    );
}
