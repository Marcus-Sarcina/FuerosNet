//! A node this client serves from, said to have moved
//! (`rhtn_client::relocation`).

mod common;

use common::harness::*;
use common::*;
use rhtn_client::payload::{PayloadConfig, PayloadState};
use rhtn_client::relocation::Found;
use rhtn_model::Standing;

const OLD: &str = "64.227.31.70:7431";
const NEW: &str = "167.172.146.119:7431";

// functional: SES-021
/// **An invitation for a node already agreed to, at another address, is a
/// relocation**, and it changes nothing until the node proves itself
/// there: then the node is used at neither address until the person
/// answers, and accepting makes the new address the agreement.
#[test]
fn a_moved_node_is_asked_about_once_it_proves_itself_and_accepted_replaces_the_agreement() {
    let mut s = setup(&["alice", "bob", "witness"]);
    let a = s.client("alice");
    let node = kh("bob");
    assert!(
        !a.offer_relocation(node, NEW.into(), None),
        "a node never agreed to is a first invitation, not a move"
    );
    a.agree_to(node, vec![OLD.into()]);
    assert!(
        !a.offer_relocation(node, OLD.into(), None),
        "the address already agreed is no move"
    );
    assert!(a.offer_relocation(node, NEW.into(), Some("Bob".into())));
    let r = a.relocation(&node).expect("held");
    assert_eq!(
        (r.standing, r.from.clone(), r.to.as_str()),
        (Standing::Unproved, vec![OLD.to_string()], NEW)
    );
    // unproved: nothing changes, the agreement is dialled as it was
    assert!(a.node_usable(&node));
    assert_eq!(a.dial_order(&node, &[OLD.into()]), vec![OLD]);
    // nothing answered yet: still unproved, asked again later
    a.relocation_proved(&node, NEW, Found::Unreached);
    assert_eq!(a.relocation(&node).unwrap().standing, Standing::Unproved);
    // a proof for an address since replaced is about nothing held
    a.relocation_proved(&node, "10.0.0.9:7431", Found::Proved);
    assert_eq!(a.relocation(&node).unwrap().standing, Standing::Unproved);
    a.relocation_proved(&node, NEW, Found::Proved);
    assert_eq!(a.relocation(&node).unwrap().standing, Standing::Proved);
    // proved: the person is asked, and neither address is used meanwhile
    assert!(!a.node_usable(&node));
    assert!(a.dial_order(&node, &[OLD.into()]).is_empty());
    assert_eq!(a.accept_relocation(&node).as_deref(), Some(NEW));
    assert_eq!(a.agreed(), vec![(node, vec![NEW.to_string()])]);
    assert!(a.relocation(&node).is_none());
    assert_eq!(a.dial_order(&node, &[NEW.into()]), vec![NEW]);
    assert!(
        a.accept_relocation(&node).is_none(),
        "nothing left to accept"
    );
}

// functional: SES-021
/// **Declining leaves the agreement and stops the node** until another
/// invitation replaces the declined one: the person cannot go on using
/// the old node as if it were active.
#[test]
fn a_declined_move_keeps_the_old_agreement_unused_until_another_invitation() {
    let mut s = setup(&["alice", "bob", "witness"]);
    let a = s.client("alice");
    let node = kh("bob");
    a.agree_to(node, vec![OLD.into()]);
    a.offer_relocation(node, NEW.into(), None);
    assert!(
        !a.decline_relocation(&node),
        "an unproved move is not put to the person, so not declined"
    );
    a.relocation_proved(&node, NEW, Found::Proved);
    assert!(a.decline_relocation(&node));
    assert_eq!(a.relocation(&node).unwrap().standing, Standing::Declined);
    assert_eq!(a.agreed(), vec![(node, vec![OLD.to_string()])]);
    assert!(!a.node_usable(&node));
    assert!(a.dial_order(&node, &[OLD.into()]).is_empty());
    // the question stays put: still a move the person may accept
    assert_eq!(a.relocation(&node).unwrap().to, NEW);
    // a new invitation replaces it, even naming the old address
    assert!(a.offer_relocation(node, OLD.into(), None));
    assert_eq!(a.relocation(&node).unwrap().standing, Standing::Unproved);
    assert!(a.node_usable(&node));
}

// functional: SES-021
/// **A declined move may still be accepted**: the notice does not clear,
/// and accepting it later is accepting it.
#[test]
fn a_declined_move_may_still_be_accepted() {
    let mut s = setup(&["alice", "bob", "witness"]);
    let a = s.client("alice");
    let node = kh("bob");
    a.agree_to(node, vec![OLD.into()]);
    a.offer_relocation(node, NEW.into(), None);
    a.relocation_proved(&node, NEW, Found::Proved);
    assert!(a.decline_relocation(&node));
    assert!(!a.decline_relocation(&node), "declined once");
    assert_eq!(a.accept_relocation(&node).as_deref(), Some(NEW));
    assert_eq!(a.agreed(), vec![(node, vec![NEW.to_string()])]);
    assert!(a.node_usable(&node));
}

// functional: SES-022
/// **Something answering for the node that is not the node stops its
/// use**, at the new address or the agreed one, until the patron sends
/// another invitation.
#[test]
fn an_address_answering_as_someone_else_stops_the_node_until_another_invitation() {
    let mut s = setup(&["alice", "bob", "witness"]);
    let a = s.client("alice");
    let node = kh("bob");
    a.agree_to(node, vec![OLD.into()]);
    a.offer_relocation(node, NEW.into(), None);
    a.relocation_proved(
        &node,
        NEW,
        Found::Irregular("delegation by another identity than the one sought".into()),
    );
    let r = a.relocation(&node).unwrap();
    assert_eq!(r.standing, Standing::Irregular);
    assert!(r.why.is_some());
    assert!(!a.node_usable(&node));
    assert!(a.accept_relocation(&node).is_none());
    assert!(!a.decline_relocation(&node));

    // at the agreed address itself, with no invitation at all
    let other = kh("witness");
    a.agree_to(other, vec![OLD.into()]);
    a.node_irregular(other, OLD.into(), "delegation does not verify".into());
    assert!(!a.node_usable(&other));
    assert_eq!(a.relocation(&other).unwrap().to, OLD);
    // and it is the patron's next invitation that resolves it
    assert!(a.offer_relocation(other, OLD.into(), None));
    a.relocation_proved(&other, OLD, Found::Proved);
    assert_eq!(a.accept_relocation(&other).as_deref(), Some(OLD));
    assert!(a.node_usable(&other));
}

// functional: SES-021
/// **The newest invitation replaces what was held**, whatever its standing.
#[test]
fn a_newer_invitation_replaces_the_one_held() {
    let mut s = setup(&["alice", "bob", "witness"]);
    let a = s.client("alice");
    let node = kh("bob");
    a.agree_to(node, vec![OLD.into()]);
    a.offer_relocation(node, NEW.into(), None);
    a.relocation_proved(&node, NEW, Found::Proved);
    assert!(a.offer_relocation(node, "10.0.0.9:7431".into(), None));
    let r = a.relocation(&node).unwrap();
    assert_eq!(
        (r.to.as_str(), r.standing),
        ("10.0.0.9:7431", Standing::Unproved)
    );
    assert_eq!(a.relocations().len(), 1, "one per node");
}

// functional: SES-021
/// **A relocation survives a restart**, every standing, and the
/// eleven-field form from before it still reads with none held.
#[test]
fn relocations_round_trip_and_the_eleven_field_form_still_reads() {
    let mut fresh = |b: &mut [u8]| b.fill(7);
    let mut p = PayloadState::new(PayloadConfig::default(), &mut fresh, 1);
    p.agreed.insert(kh("n1"), vec![OLD.to_string()]);
    let eleven = p.encode();
    for (i, standing) in [
        Standing::Unproved,
        Standing::Proved,
        Standing::Declined,
        Standing::Irregular,
    ]
    .into_iter()
    .enumerate()
    {
        p.relocations.insert(
            kh(["n1", "n2", "n3", "n4"][i]),
            rhtn_client::relocation::Held {
                to: NEW.to_string(),
                standing,
                why: (standing == Standing::Irregular).then(|| "not the node".to_string()),
                label: (i == 1).then(|| "Bob".to_string()),
            },
        );
    }
    let bytes = p.encode();
    assert_eq!(bytes[0], 0x8c, "twelve fields");
    let back = PayloadState::decode(PayloadConfig::default(), &bytes).expect("reads");
    assert_eq!(back.relocations, p.relocations);
    // the form before relocations: written without the twelfth field
    let mut old_form = eleven.clone();
    assert_eq!(old_form[0], 0x8c);
    // drop the trailing empty array and say eleven
    assert_eq!(
        old_form.pop(),
        Some(0x80),
        "the empty relocation list is last"
    );
    old_form[0] = 0x8b;
    let back = PayloadState::decode(PayloadConfig::default(), &old_form).expect("reads");
    assert!(back.relocations.is_empty());
    assert_eq!(back.agreed, p.agreed);
}

// functional: SES-021
/// **A subnet names the patron whose node serves it, and carries the
/// move held for that node**, which is what the shell draws on its orb.
#[test]
fn a_subnet_carries_its_patron_and_the_move_held_for_its_node() {
    let mut s = setup(&["alice", "bob", "witness"]);
    s.adopted("alice", "bob", "witness", 1);
    let a = s.client("alice");
    let held = a.subnets_held();
    let row = held
        .iter()
        .find(|r| r.patron.is_some())
        .expect("a subnet under bob");
    assert_eq!(row.patron, Some(kh("bob")));
    assert!(row.relocation.is_none());
    a.agree_to(kh("bob"), vec![OLD.into()]);
    a.offer_relocation(kh("bob"), NEW.into(), Some("Bob".into()));
    a.relocation_proved(&kh("bob"), NEW, Found::Proved);
    let held = a.subnets_held();
    let row = held.iter().find(|r| r.patron == Some(kh("bob"))).unwrap();
    let r = row.relocation.as_ref().expect("the move rides the subnet");
    assert_eq!((r.standing, r.to.as_str()), (Standing::Proved, NEW));
}
