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
/// **An address that answers as someone else changes nothing for the
/// person**: the node goes on being used, nothing is put to them, and the
/// failed invitation waits only until the operator has been told.
#[test]
fn an_address_answering_as_someone_else_changes_nothing_and_waits_to_be_reported() {
    let mut s = setup(&["alice", "bob", "witness"]);
    let a = s.client("alice");
    let node = kh("bob");
    a.agree_to(node, vec![OLD.into()]);
    a.offer_relocation(node, NEW.into(), None);
    a.relocation_proved(
        &node,
        NEW,
        Found::Failed("delegation by another identity than the one sought".into()),
    );
    let r = a.relocation(&node).unwrap();
    assert_eq!(r.standing, Standing::Failed);
    assert!(r.why.is_some());
    assert!(a.node_usable(&node), "a link shuts nothing");
    assert_eq!(a.dial_order(&node, &[OLD.into()]), vec![OLD]);
    assert!(a.accept_relocation(&node).is_none());
    assert!(!a.decline_relocation(&node));
    assert_eq!(a.agreed(), vec![(node, vec![OLD.to_string()])]);
    // told, and gone
    assert!(a.failure_reported(&node));
    assert!(a.relocation(&node).is_none());
    assert!(!a.failure_reported(&node), "nothing left to report");
    // a proved move is not a failure to report
    a.offer_relocation(node, NEW.into(), None);
    a.relocation_proved(&node, NEW, Found::Proved);
    assert!(!a.failure_reported(&node));
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
        Standing::Failed,
    ]
    .into_iter()
    .enumerate()
    {
        p.relocations.insert(
            kh(["n1", "n2", "n3", "n4"][i]),
            rhtn_client::relocation::Held {
                to: NEW.to_string(),
                standing,
                why: (standing == Standing::Failed).then(|| "not the node".to_string()),
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

// functional: SES-021
/// **A node serving this client two levels down is invited by name, and
/// its move rides the subnet it serves** (SES-002): an invitation naming
/// an agreed node that is not this client's patron is placed as that node,
/// and the subnet whose address that node serves carries the move.
#[test]
fn a_node_two_levels_up_is_invited_by_name_and_its_move_rides_the_subnet_it_serves() {
    use rhtn_client::invitation::{Invitation, Placed};
    let mut s = setup(&["alice", "bob", "witness"]);
    s.adopted("alice", "bob", "witness", 1);
    let a = s.client("alice");
    let far = kh("witness");
    let named = Invitation {
        at: NEW.into(),
        patron: Some(far),
        subnet: None,
        member: None,
        as_label: None,
    };
    assert_eq!(
        a.place_invitation(&named),
        Placed::NotAPatron(far),
        "a party never agreed to is not introduced"
    );
    a.agree_to(far, vec![OLD.into()]);
    assert_eq!(a.place_invitation(&named), Placed::Node(far));
    let row = a
        .subnets_held()
        .into_iter()
        .find(|r| r.patron.is_some())
        .unwrap();
    let anchor = row.position.as_ref().unwrap().anchor;
    a.serve_at(anchor, far);
    a.offer_relocation(far, NEW.into(), None);
    a.relocation_proved(&far, NEW, Found::Proved);
    let row = a
        .subnets_held()
        .into_iter()
        .find(|r| r.id == row.id)
        .unwrap();
    assert_eq!(row.patron, Some(kh("bob")), "the line is still bob's");
    assert_eq!(
        row.relocation.map(|r| r.node),
        Some(far),
        "the move of the node serving it"
    );
}
