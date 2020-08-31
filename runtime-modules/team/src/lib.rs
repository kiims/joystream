// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

// Do not delete! Cannot be uncommented by default, because of Parity decl_module! issue.
//#![warn(missing_docs)]

mod checks;
mod errors;
#[cfg(test)]
mod tests;
mod types;

use codec::Codec;
use frame_support::dispatch::DispatchResult;
//use frame_support::storage::IterableStorageMap;
//use frame_support::traits::{Currency, ExistenceRequirement, Get, Imbalance, WithdrawReasons};
use frame_support::traits::Get;
use frame_support::{decl_event, decl_module, decl_storage, Parameter, StorageValue}; // ensure, print,
use sp_arithmetic::traits::{BaseArithmetic, One};
use sp_runtime::traits::{Hash, MaybeSerialize, Member};
// use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
// use sp_std::vec;
// use sp_std::vec::Vec;
// use system::{ensure_root, ensure_signed};

use common::constraints::InputValidationLengthConstraint;

pub use errors::Error;
pub use types::{JobOpening, JobOpeningType};

/// The _Team_ main _Trait_
pub trait Trait<I: Instance>: system::Trait {
    /// OpeningId type
    type OpeningId: Parameter
        + Member
        + BaseArithmetic
        + Codec
        + Default
        + Copy
        + MaybeSerialize
        + PartialEq;

    /// _Administration_ event type.
    type Event: From<Event<Self, I>> + Into<<Self as system::Trait>::Event>;

    /// Defines max workers number in the team.
    type MaxWorkerNumberLimit: Get<u32>;
}

decl_event!(
    /// _Team_ events
    pub enum Event<T, I>
    where
       <T as Trait<I>>::OpeningId,
    {
        /// Emits on adding new job opening.
        /// Params:
        /// - Opening id
        OpeningAdded(OpeningId),
    }
);

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance> as Team {
        /// Next identifier value for new job opening.
        pub NextOpeningId get(fn next_opening_id): T::OpeningId;

        /// Maps identifier to job opening.
        pub OpeningById get(fn opening_by_id): map hasher(blake2_128_concat)
            T::OpeningId => JobOpening<T::BlockNumber>;

        /// Count of active workers.
        pub ActiveWorkerCount get(fn active_worker_count): u32;

        /// Opening human readable text length limits.
        pub OpeningDescriptionTextLimit get(fn opening_description_text_limit):
            InputValidationLengthConstraint;
    }
    add_extra_genesis {
//        config(phantom): sp_std::marker::PhantomData<I>;
        config(opening_description_constraint): InputValidationLengthConstraint;
        build(|config: &GenesisConfig| {
            Module::<T, I>::initialize_working_group(config.opening_description_constraint);
        });
    }
}

decl_module! {
    /// _Working group_ substrate module.
    pub struct Module<T: Trait<I>, I: Instance> for enum Call where origin: T::Origin {
        /// Default deposit_event() handler
        fn deposit_event() = default;

        /// Predefined errors
        type Error = Error<T, I>;

        /// Exports const -  max simultaneous active worker number.
        const MaxWorkerNumberLimit: u32 = T::MaxWorkerNumberLimit::get();

        // ****************** Hiring flow **********************

        /// Add a job opening for a worker role.
        /// Require signed leader origin or the root (to add opening for the leader position).
        #[weight = 10_000_000] // TODO: adjust weight
        pub fn add_opening(
            origin,
            description: Vec<u8>,
            opening_type: JobOpeningType,
        ){
            checks::ensure_origin_for_opening_type::<T, I>(origin, opening_type)?;

            checks::ensure_opening_description_is_valid::<T, I>(&description)?;

            // Self::ensure_opening_policy_commitment_is_valid(&commitment)?;


            //
            // == MUTATION SAFE ==
            //

           let hashed_description = T::Hashing::hash(&description);

            // Create and add worker opening.
            let new_opening = JobOpening::<T::BlockNumber> {
//                applications: BTreeSet::new(),
//                policy_commitment,
                opening_type,
                created: Self::current_block(),
                description_hash: hashed_description.as_ref().to_vec(),
                is_active: true,
            };

             let new_opening_id = NextOpeningId::<T, I>::get();

            OpeningById::<T, I>::insert(new_opening_id, new_opening);

            // Update NextOpeningId
            NextOpeningId::<T, I>::mutate(|id| *id += <T::OpeningId as One>::one());

            Self::deposit_event(RawEvent::OpeningAdded(new_opening_id));
        }
    }
}

impl<T: Trait<I>, I: Instance> Module<T, I> {
    // Initialize working group constraints and mint.
    pub(crate) fn initialize_working_group(
        opening_description_constraint: InputValidationLengthConstraint,
    ) {
        // Create constraints
        <OpeningDescriptionTextLimit<I>>::put(opening_description_constraint);
    }

    // Wrapper-function over system::block_number()
    fn current_block() -> T::BlockNumber {
        <system::Module<T>>::block_number()
    }
}
