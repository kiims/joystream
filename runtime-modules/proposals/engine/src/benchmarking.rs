#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::dummy_proposals;
use crate::Module as ProposalsEngine;
use balances::Module as Balances;
use core::convert::TryInto;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::{Currency, OnInitialize};
use frame_system::EventRecord;
use frame_system::Module as System;
use frame_system::RawOrigin;
use governance::council::Module as Council;
use membership::Module as Membership;
use sp_runtime::traits::{Bounded, One};
use sp_std::cmp::min;
use sp_std::prelude::*;

use codec::Encode;

const SEED: u32 = 0;

fn get_byte(num: u32, byte_number: u8) -> u8 {
    ((num & (0xff << (8 * byte_number))) >> 8 * byte_number) as u8
}

// Method to generate a distintic valid handle
// for a membership. For each index.
fn handle_from_id<T: membership::Trait>(id: u32) -> Vec<u8> {
    let min_handle_length = Membership::<T>::min_handle_length();

    let mut handle = vec![];

    for i in 0..min(Membership::<T>::max_handle_length().try_into().unwrap(), 4) {
        handle.push(get_byte(id, i));
    }

    while handle.len() < (min_handle_length as usize) {
        handle.push(0u8);
    }

    handle
}

fn assert_last_event<T: Trait>(generic_event: <T as Trait>::Event) {
    let events = System::<T>::events();
    let system_event: <T as frame_system::Trait>::Event = generic_event.into();
    assert!(
        events.len() > 0,
        "If you are checking for last event there must be at least 1 event"
    );
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn assert_in_events<T: Trait>(generic_event: <T as Trait>::Event) {
    let events = System::<T>::events();
    let system_event: <T as frame_system::Trait>::Event = generic_event.into();

    assert!(
        events.len() > 0,
        "If you are checking for last event there must be at least 1 event"
    );

    assert!(events.iter().any(|event| {
        let EventRecord { event, .. } = event;
        event == &system_event
    }));
}

fn member_funded_account<T: Trait>(name: &'static str, id: u32) -> (T::AccountId, T::MemberId) {
    let account_id = account::<T::AccountId>(name, id, SEED);
    let handle = handle_from_id::<T>(id);

    let authority_account = account::<T::AccountId>(name, 0, SEED);

    Membership::<T>::set_screening_authority(RawOrigin::Root.into(), authority_account.clone())
        .unwrap();

    Membership::<T>::add_screened_member(
        RawOrigin::Signed(authority_account.clone()).into(),
        account_id.clone(),
        Some(handle),
        None,
        None,
    )
    .unwrap();

    let _ = Balances::<T>::make_free_balance_be(&account_id, T::Balance::max_value());

    (account_id, T::MemberId::from(id.try_into().unwrap()))
}

#[derive(Copy, Clone)]
enum CallType {
    Passing,
    Failing,
    FailingDecode,
}

fn create_proposal<T: Trait + dummy_proposals::Trait>(
    id: u32,
    proposal_number: u32,
    constitutionality: u32,
    call_type: CallType,
) -> (T::AccountId, T::MemberId, T::ProposalId) {
    let (account_id, member_id) = member_funded_account::<T>("member", id);

    let proposal_parameters = ProposalParameters {
        voting_period: T::BlockNumber::from(1),
        grace_period: Zero::zero(),
        approval_quorum_percentage: 1,
        approval_threshold_percentage: 1,
        slashing_quorum_percentage: 0,
        slashing_threshold_percentage: 1,
        required_stake: Some(T::Balance::max_value()),
        constitutionality,
    };

    let encoded_call = match call_type {
        CallType::Passing => dummy_proposals::Call::<T>::dummy_proposal(vec![], vec![]).encode(),
        CallType::Failing => dummy_proposals::Call::<T>::faulty_proposal(vec![], vec![]).encode(),
        CallType::FailingDecode => vec![],
    };

    let proposal_creation_parameters = ProposalCreationParameters {
        account_id: account_id.clone(),
        proposer_id: member_id.clone(),
        proposal_parameters,
        title: vec![0u8],
        description: vec![0u8],
        staking_account_id: Some(account_id.clone()),
        encoded_dispatchable_call_code: encoded_call.clone(),
        exact_execution_block: None,
    };

    let proposal_id = ProposalsEngine::<T>::create_proposal(proposal_creation_parameters).unwrap();

    assert!(
        Proposals::<T>::contains_key(proposal_id),
        "Proposal not created"
    );
    assert!(
        DispatchableCallCode::<T>::contains_key(proposal_id),
        "Dispatchable code not added"
    );

    assert_eq!(
        ProposalsEngine::<T>::proposal_codes(proposal_id),
        encoded_call,
        "Dispatchable code does not match"
    );

    assert_eq!(
        ProposalsEngine::<T>::proposal_count(),
        proposal_number,
        "Not correct number of proposals stored"
    );

    // We assume here that active proposals == number of proposals
    assert_eq!(
        ProposalsEngine::<T>::active_proposal_count(),
        proposal_number,
        "Created proposal not active"
    );

    (account_id, member_id, proposal_id)
}

fn create_multiple_finalized_proposals<
    T: Trait + governance::council::Trait + dummy_proposals::Trait,
>(
    number_of_proposals: u32,
    constitutionality: u32,
    call_type: CallType,
    vote_kind: VoteKind,
    total_voters: u32,
) -> (Vec<T::AccountId>, Vec<T::ProposalId>) {
    let mut voters = Vec::new();
    for i in 0..total_voters {
        voters.push(member_funded_account::<T>("voter", i));
    }

    Council::<T>::set_council(
        RawOrigin::Root.into(),
        voters
            .iter()
            .map(|(account_id, _)| account_id.clone())
            .collect(),
    )
    .unwrap();

    let mut proposers = Vec::new();
    let mut proposals = Vec::new();
    for id in total_voters..number_of_proposals + total_voters {
        let (proposer_account_id, _, proposal_id) =
            create_proposal::<T>(id, id - total_voters + 1, constitutionality, call_type);
        proposers.push(proposer_account_id);
        proposals.push(proposal_id);

        for (voter_id, member_id) in voters.clone() {
            ProposalsEngine::<T>::vote(
                RawOrigin::Signed(voter_id.clone()).into(),
                member_id,
                proposal_id,
                vote_kind.clone(),
                vec![0u8],
            )
            .unwrap()
        }
    }

    (proposers, proposals)
}

const MAX_BYTES: u32 = 16384;

// In version 2.0 of substrate `T::MaxLocks` was added to balance
// see: https://github.com/paritytech/substrate/pull/7103/commits/20a77424686b169d254b542ec4b128dce6a4ef8b
// update this benchmark when updating.
const MAX_LOCKS: u32 = 125;

benchmarks! {
    // Note: this is the syntax for this macro can't use "+"
    where_clause {
        where T: governance::council::Trait, T: dummy_proposals::Trait
    }

    _ { }

    vote {
        let i in 0 .. MAX_BYTES;

        let (_, _, proposal_id) = create_proposal::<T>(0, 1, 0, CallType::Passing);

        let (account_voter_id, member_voter_id) = member_funded_account::<T>("voter", 1);

        Council::<T>::set_council(RawOrigin::Root.into(), vec![account_voter_id.clone()]).unwrap();
    }: _ (
            RawOrigin::Signed(account_voter_id),
            member_voter_id,
            proposal_id,
            VoteKind::Approve,
            vec![0u8; i.try_into().unwrap()]
        )
    verify {
        assert!(Proposals::<T>::contains_key(proposal_id), "Proposal should still exist");

        let voting_results = ProposalsEngine::<T>::proposals(proposal_id).voting_results;

        assert_eq!(
          voting_results,
          VotingResults{ approvals: 1, abstentions: 0, rejections: 0, slashes: 0 },
          "There should only be 1 approval"
        );

        assert!(
          VoteExistsByProposalByVoter::<T>::contains_key(proposal_id, member_voter_id),
          "Voter not added to existing voters"
        );

        assert_eq!(
          ProposalsEngine::<T>::vote_by_proposal_by_voter(proposal_id, member_voter_id),
          VoteKind::Approve,
          "Stored vote doesn't match"
        );

        assert_last_event::<T>(
            RawEvent::Voted(member_voter_id, proposal_id, VoteKind::Approve).into()
        );
    }

    cancel_proposal {
        let i in 1 .. MAX_LOCKS;

        let (account_id, member_id, proposal_id) = create_proposal::<T>(0, 1, 0, CallType::Passing);

        for lock_number in 1 .. i {
            let (locked_account_id, _) = member_funded_account::<T>("locked_member", lock_number);
            T::StakingHandler::set_stake(&locked_account_id, One::one()).unwrap();
        }

    }: _ (RawOrigin::Signed(account_id.clone()), member_id, proposal_id)
    verify {
        assert!(!Proposals::<T>::contains_key(proposal_id), "Proposal still in storage");

        assert!(
            !DispatchableCallCode::<T>::contains_key(proposal_id),
            "Proposal code still in storage"
        );

        assert_eq!(ProposalsEngine::<T>::active_proposal_count(), 0, "Proposal still active");

        assert_eq!(
            Balances::<T>::usable_balance(account_id),
            T::Balance::max_value() - T::CancellationFee::get(),
            "Balance not slashed"
        );

        assert_last_event::<T>(
            RawEvent::ProposalDecisionMade(proposal_id, ProposalDecision::Canceled).into()
        );
    }

    veto_proposal {
        let i in 0 .. 1;
        let (account_id, _, proposal_id) = create_proposal::<T>(0, 1, 0, CallType::Passing);
    }: _ (RawOrigin::Root, proposal_id)
    verify {
        assert!(!Proposals::<T>::contains_key(proposal_id), "Proposal still in storage");

        assert!(
            !DispatchableCallCode::<T>::contains_key(proposal_id),
            "Proposal code still in storage"
        );

        assert_eq!(ProposalsEngine::<T>::active_proposal_count(), 0, "Proposal still active");

        assert_eq!(
            Balances::<T>::usable_balance(account_id),
            T::Balance::max_value(),
            "Vetoed proposals shouldn't be slashed"
        );

        assert_last_event::<T>(
            RawEvent::ProposalDecisionMade(proposal_id, ProposalDecision::Vetoed).into()
        );
    }

    on_initialize_immediate_execution {
        let i in 1 .. T::MaxActiveProposalLimit::get();

        let (proposers, proposals) = create_multiple_finalized_proposals::<T>(
            i,
            0,
            CallType::Passing,
            VoteKind::Approve,
            1,
        );

    }: { ProposalsEngine::<T>::on_initialize(System::<T>::block_number().into()) }
    verify {
        for proposer_account_id in proposers {
            assert_eq!(
                T::StakingHandler::current_stake(&proposer_account_id),
                Zero::zero(),
                "Should've unlocked all stake"
            );
        }

        /*
        for proposal_id in proposals.iter() {
            assert_in_events::<T>(
                RawEvent::ProposalExecuted(proposal_id.clone(), ExecutionStatus::Executed).into()
            );
        }
        */
    }

    on_initialize_immediate_execution_decode_fails {
        let i in 1 .. T::MaxActiveProposalLimit::get();

        let (proposers, proposals) = create_multiple_finalized_proposals::<T>(
            i,
            0,
            CallType::FailingDecode,
            VoteKind::Approve,
            1
        );
    }: { ProposalsEngine::<T>::on_initialize(System::<T>::block_number().into()) }
    verify {
        for proposer_account_id in proposers {
            assert_eq!(
                T::StakingHandler::current_stake(&proposer_account_id),
                Zero::zero(),
                "Should've unlocked all stake"
            );
        }

        for proposal_id in proposals.iter() {
            assert_in_events::<T>(
                RawEvent::ProposalExecuted(proposal_id.clone(), ExecutionStatus::failed_execution("Not enough data to fill buffer")).into()
            );
        }
    }

    on_initialize_immediate_execution_fails {
        let i in 1 .. T::MaxActiveProposalLimit::get();

        let (proposers, proposals) = create_multiple_finalized_proposals::<T>(
            i,
            0,
            CallType::Failing,
            VoteKind::Approve,
            1
        );
    }: { ProposalsEngine::<T>::on_initialize(System::<T>::block_number().into()) }
    verify {
        for proposer_account_id in proposers {
            assert_eq!(
                T::StakingHandler::current_stake(&proposer_account_id),
                Zero::zero(),
                "Should've unlocked all stake"
            );
        }

        for proposal_id in proposals.iter() {
            assert_in_events::<T>(
                RawEvent::ProposalExecuted(proposal_id.clone(), ExecutionStatus::failed_execution("ExecutionFailed")).into()
            );
        }
    }

    on_initialize_approved_pending_constitutionality {
        let i in 1 .. T::MaxActiveProposalLimit::get();

        let (proposers, proposals) = create_multiple_finalized_proposals::<T>(
            i,
            2,
            CallType::Passing,
            VoteKind::Approve,
            1
        );

    }: { ProposalsEngine::<T>::on_initialize(System::<T>::block_number().into()) }
    verify {
        for proposer_account_id in proposers {
            assert_ne!(
                T::StakingHandler::current_stake(&proposer_account_id),
                Zero::zero(),
                "Should've still stake locked"
            );
        }

        for proposal_id in proposals.iter() {

            assert!(Proposals::<T>::contains_key(proposal_id), "Proposal should still be in the store");
            let proposal = ProposalsEngine::<T>::proposals(proposal_id);
            let status = ProposalStatus::approved(ApprovedProposalDecision::PendingConstitutionality ,System::<T>::block_number());
            assert_eq!(proposal.status, status);
            assert_eq!(proposal.current_constitutionality_level, 1);
            assert_in_events::<T>(
                RawEvent::ProposalStatusUpdated(proposal_id.clone(), status).into()
            );
        }
    }

    on_initialize_rejected {
        let i in 1 .. T::MaxActiveProposalLimit::get();

        let (proposers, proposals) = create_multiple_finalized_proposals::<T>(
            i,
            0,
            CallType::Passing,
            VoteKind::Reject,
            T::TotalVotersCounter::total_voters_count(),
        );
    }: { ProposalsEngine::<T>::on_initialize(System::<T>::block_number().into()) }
    verify {
        for proposer_account_id in proposers {
            assert_eq!(
                T::StakingHandler::current_stake(&proposer_account_id),
                Zero::zero(),
                "Shouldn't have any stake locked"
            );
        }

        for proposal_id in proposals.iter() {

            assert!(!Proposals::<T>::contains_key(proposal_id), "Proposal should not be in store");
            assert!(!DispatchableCallCode::<T>::contains_key(proposal_id), "Dispatchable should not be in store");
            assert_in_events::<T>(
                RawEvent::ProposalDecisionMade(proposal_id.clone(), ProposalDecision::Rejected).into()
            );
        }

        assert_eq!(ProposalsEngine::<T>::active_proposal_count(), 0, "There should not be any proposal left active");
    }

    on_initialize_slashed {
        let i in 1 .. T::MaxActiveProposalLimit::get();

        let (proposers, proposals) = create_multiple_finalized_proposals::<T>(
            i,
            0,
            CallType::Passing,
            VoteKind::Slash,
            T::TotalVotersCounter::total_voters_count(),
        );
    }: { ProposalsEngine::<T>::on_initialize(System::<T>::block_number().into()) }
    verify {
        for proposer_account_id in proposers {
            assert_eq!(
                T::StakingHandler::current_stake(&proposer_account_id),
                Zero::zero(),
                "Shouldn't have any stake locked"
            );

            assert_eq!(Balances::<T>::free_balance(&proposer_account_id), Zero::zero(), "Should've all balance slashed");
        }

        for proposal_id in proposals.iter() {

            assert!(!Proposals::<T>::contains_key(proposal_id), "Proposal should not be in store");
            assert!(!DispatchableCallCode::<T>::contains_key(proposal_id), "Dispatchable should not be in store");
            assert_in_events::<T>(
                RawEvent::ProposalDecisionMade(proposal_id.clone(), ProposalDecision::Slashed).into()
            );
        }

        assert_eq!(ProposalsEngine::<T>::active_proposal_count(), 0, "There should not be any proposal left active");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::mock::{initial_test_ext, Test};
    use frame_support::assert_ok;

    #[test]
    fn test_vote() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_vote::<Test>());
        });
    }

    #[test]
    fn test_cancel_proposal() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_cancel_proposal::<Test>());
        });
    }

    #[test]
    fn test_veto_proposal() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_veto_proposal::<Test>());
        });
    }

    #[test]
    fn test_on_initialize_immediate_execution() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_immediate_execution::<Test>());
        });
    }

    #[test]
    fn test_on_initialize_immediate_execution_decode_fails() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_immediate_execution_decode_fails::<Test>());
        });
    }

    #[test]
    fn test_on_initialize_immediate_execution_fails() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_immediate_execution_fails::<Test>());
        });
    }

    #[test]
    fn test_on_initialize_approved_pending_constitutionality() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_approved_pending_constitutionality::<Test>());
        });
    }

    #[test]
    fn test_on_initialize_rejected() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_rejected::<Test>());
        });
    }

    #[test]
    fn test_on_initialize_slashed() {
        initial_test_ext().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_slashed::<Test>());
        });
    }
}
