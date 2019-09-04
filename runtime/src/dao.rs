use support::{
    decl_event, decl_module, decl_storage, dispatch::Result, ensure, traits::Currency, StorageMap,
    StorageValue,
};
use system::ensure_signed;

use runtime_primitives::traits::{As, Hash};

use parity_codec::{Decode, Encode};

use rstd::prelude::Vec;

/// The module's configuration trait.
pub trait Trait: balances::Trait + timestamp::Trait + system::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type Count = u64;
pub type DaoId = u64;
pub type MemberId = u64;
pub type ProposalId = u64;
pub type VotesCount = MemberId;

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Dao<AccountId> {
    address: AccountId,
    name: Vec<u8>,
    description: Vec<u8>,
    founder: AccountId,
}

#[derive(Encode, Decode, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Proposal<DaoId, AccountId, VotingDeadline, MemberId> {
    dao_id: DaoId,
    action: Action<AccountId>,
    open: bool,
    voting_deadline: VotingDeadline,
    yes_count: MemberId,
    no_count: MemberId,
}

impl<D, A, V, M> Default for Proposal<D, A, V, M>
where
    D: Default,
    A: Default,
    V: Default,
    M: Default,
{
    fn default() -> Self {
        Proposal {
            dao_id: D::default(),
            action: Action::EmptyAction,
            open: true,
            voting_deadline: V::default(),
            yes_count: M::default(),
            no_count: M::default(),
        }
    }
}

#[derive(Encode, Decode, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Action<AccountId> {
    EmptyAction,
    AddMember(AccountId),
    RemoveMember(AccountId),
}

/// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as DaoStorage {
        Daos get(daos): map(DaoId) => Dao<T::AccountId>;
        DaosCount get(daos_count): Count;
        DaoNames get(dao_names): map(T::Hash) => DaoId;
        DaoAddresses get(dao_addresses): map(T::AccountId) => DaoId;
        Address get(address): map(DaoId) => T::AccountId;

        MaximumNumberOfMebers get(maximum_number_of_members) config(): MemberId = 4;
        Members get(members): map(DaoId, MemberId) => T::AccountId;
        MembersCount get(members_count): map(DaoId) => MemberId;
        DaoMembers get(dao_members): map(DaoId, T::AccountId) => MemberId;

        DaoProposalsPeriodLimit get(dao_proposals_period_limit) config(): T::BlockNumber = T::BlockNumber::sa(30);
        DaoProposals get(dao_proposals): map(DaoId, ProposalId) => Proposal<DaoId, T::AccountId, T::BlockNumber, VotesCount>;
        DaoProposalsCount get(dao_proposals_count): map(DaoId) => ProposalId;
        DaoProposalsIndex get(dao_proposals_index): map(ProposalId) => DaoId;

        DaoProposalsVotes get(dao_proposals_votes): map(DaoId, ProposalId, MemberId) => T::AccountId;
        DaoProposalsVotesCount get(dao_proposals_votes_count): map(DaoId, ProposalId) => MemberId;
        DaoProposalsVotesIndex get(dao_proposals_votes_index): map(DaoId, ProposalId, T::AccountId) => MemberId;

        OpenDaoProposalsLimit get(open_proposals_per_block) config(): usize = 2;
        OpenDaoProposals get(open_dao_proposals): map(T::BlockNumber) => Vec<ProposalId>;
        OpenDaoProposalsIndex get(open_dao_proposals_index): map(ProposalId) => T::BlockNumber;
        OpenDaoProposalsHashes get(open_dao_proposals_hashes): map(T::Hash) => ProposalId;
        OpenDaoProposalsHashesIndex get(open_dao_proposals_hashes_index): map(ProposalId) => T::Hash;
    }
}

decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event<T>() = default;

        pub fn create(origin, address: T::AccountId, name: Vec<u8>, description: Vec<u8>) -> Result {
            let founder = ensure_signed(origin)?;

            let daos_count = <DaosCount<T>>::get();
            let new_daos_count = daos_count
                .checked_add(1)
                .ok_or("Overflow adding a new dao")?;
            let name_hash = (&name).using_encoded(<T as system::Trait>::Hashing::hash);

            Self::validate_name(&name)?;
            Self::validate_description(&description)?;
            ensure!(!<DaoAddresses<T>>::exists(&address), "This DAO address already busy");
            ensure!(!<DaoNames<T>>::exists(&name_hash), "This DAO name already exists");
            // TODO: should the total balance of DAO address be zero?

            let new_dao = Dao {
                address: address.clone(),
                name: name.clone(),
                description,
                founder: founder.clone()
            };
            let dao_id = daos_count;

            let dao_deposit = <balances::ExistentialDeposit<T>>::get();
            <balances::Module<T> as Currency<_>>::transfer(&founder, &address, dao_deposit)?;

            <Daos<T>>::insert(dao_id, new_dao);
            <DaosCount<T>>::put(new_daos_count);
            <DaoNames<T>>::insert(name_hash, dao_id);
            <DaoAddresses<T>>::insert(&address, dao_id);
            <Address<T>>::insert(dao_id, &address);
            <Members<T>>::insert((dao_id, 0), &founder);
            <MembersCount<T>>::insert(dao_id, 1);
            <DaoMembers<T>>::insert((dao_id, founder.clone()), 0);

            Self::deposit_event(RawEvent::DaoCreated(address, founder, name));
            Ok(())
        }

        pub fn propose_to_add_member(origin, dao_id: DaoId) -> Result {
            let candidate = ensure_signed(origin)?;

            let proposal_hash = ("propose_to_add_member", &candidate, dao_id)
                .using_encoded(<T as system::Trait>::Hashing::hash);
            let voting_deadline = <system::Module<T>>::block_number() + Self::dao_proposals_period_limit();
            let mut open_proposals = Self::open_dao_proposals(voting_deadline);

            ensure!(<Daos<T>>::exists(dao_id), "This DAO not exists");
            ensure!(!<DaoMembers<T>>::exists((dao_id, candidate.clone())), "You already are a member of this DAO");
            ensure!(!<DaoAddresses<T>>::exists(candidate.clone()), "A DAO can not be a member of other DAO");
            ensure!(<MembersCount<T>>::get(dao_id) + 1 <= Self::maximum_number_of_members(), "Maximum number of members for this DAO is reached");
            ensure!(!<OpenDaoProposalsHashes<T>>::exists(proposal_hash), "This proposal already open");
            ensure!(open_proposals.len() < Self::open_proposals_per_block(), "Maximum number of open proposals is reached for the target block, try later");
            // TODO: maybe need to limit new candidates by the minimum total balance?

            let dao_proposals_count = <DaoProposalsCount<T>>::get(dao_id);
            let new_dao_proposals_count = dao_proposals_count
                .checked_add(1)
                .ok_or("Overflow adding a new DAO proposal")?;

            let proposal = Proposal {
                dao_id,
                action: Action::AddMember(candidate.clone()),
                open: true,
                voting_deadline,
                yes_count: 0,
                no_count: 0
            };
            let proposal_id = dao_proposals_count;
            open_proposals.push(proposal_id);

            <DaoProposals<T>>::insert((dao_id, proposal_id), proposal);
            <DaoProposalsCount<T>>::insert(dao_id, new_dao_proposals_count);
            <DaoProposalsIndex<T>>::insert(proposal_id, dao_id);
            <OpenDaoProposals<T>>::insert(voting_deadline, open_proposals);
            <OpenDaoProposalsHashes<T>>::insert(proposal_hash, proposal_id);
            <OpenDaoProposalsHashesIndex<T>>::insert(proposal_id, proposal_hash);

            Self::deposit_event(RawEvent::ProposeToAddMember(dao_id, candidate, voting_deadline));
            Ok(())
        }

        pub fn propose_to_remove_member(origin, dao_id: DaoId) -> Result {
            let candidate = ensure_signed(origin)?;

            let proposal_hash = ("propose_to_remove_member", &candidate, dao_id)
                .using_encoded(<T as system::Trait>::Hashing::hash);
            let voting_deadline = <system::Module<T>>::block_number() + Self::dao_proposals_period_limit();
            let mut open_proposals = Self::open_dao_proposals(voting_deadline);

            ensure!(<Daos<T>>::exists(dao_id), "This DAO not exists");
            ensure!(<DaoMembers<T>>::exists((dao_id, candidate.clone())), "You already are not a member of this DAO");
            ensure!(<MembersCount<T>>::get(dao_id) > 1, "You are the latest member of this DAO");
            ensure!(!<OpenDaoProposalsHashes<T>>::exists(proposal_hash), "This proposal already open");
            ensure!(open_proposals.len() < Self::open_proposals_per_block(), "Maximum number of open proposals is reached for the target block, try later");

            let dao_proposals_count = <DaoProposalsCount<T>>::get(dao_id);
            let new_dao_proposals_count = dao_proposals_count
                .checked_add(1)
                .ok_or("Overflow adding a new DAO proposal")?;

            let proposal = Proposal {
                dao_id,
                action: Action::RemoveMember(candidate.clone()),
                open: true,
                voting_deadline,
                yes_count: 0,
                no_count: 0
            };
            let proposal_id = dao_proposals_count;
            open_proposals.push(proposal_id);

            <DaoProposals<T>>::insert((dao_id, new_dao_proposals_count), proposal);
            <DaoProposalsCount<T>>::insert(dao_id, dao_proposals_count);
            <DaoProposalsIndex<T>>::insert(proposal_id, dao_id);
            <OpenDaoProposals<T>>::insert(voting_deadline, open_proposals);
            <OpenDaoProposalsHashes<T>>::insert(proposal_hash, proposal_id);
            <OpenDaoProposalsHashesIndex<T>>::insert(proposal_id, proposal_hash);

            Self::deposit_event(RawEvent::ProposeToRemoveMember(dao_id, candidate, voting_deadline));
            Ok(())
        }

        pub fn vote(origin, dao_id: DaoId, proposal_id: ProposalId, vote: bool) -> Result {
            let voter = ensure_signed(origin)?;

            ensure!(<DaoMembers<T>>::exists((dao_id, voter.clone())), "You are not a member of this DAO");
            ensure!(<DaoProposals<T>>::exists((dao_id, proposal_id)), "This proposal not exists");
            ensure!(!<DaoProposalsVotesIndex<T>>::exists((dao_id, proposal_id, voter.clone())), "You voted already");

            let dao_proposal_votes_count = <DaoProposalsVotesCount<T>>::get((dao_id, proposal_id));
            let new_dao_proposals_votes_count = dao_proposal_votes_count
                .checked_add(1)
                .ok_or("Overwlow adding a new vote of DAO proposal")?;

            let mut proposal = <DaoProposals<T>>::get((dao_id, proposal_id));
            ensure!(proposal.open, "This proposal is not open");

            if vote {
                proposal.yes_count += 1;
            } else {
                proposal.no_count += 1;
            }

            let dao_members_count = <MembersCount<T>>::get(dao_id);
            let proposal_is_accepted = Self::votes_are_enough(proposal.yes_count, dao_members_count);
            let proposal_is_rejected = Self::votes_are_enough(proposal.no_count, dao_members_count);

            if proposal_is_accepted {
                Self::execute_proposal(&proposal)?;
            }

            if proposal_is_accepted || proposal_is_rejected {
                Self::close_proposal(dao_id, proposal_id, proposal.clone());
            } else {
                <DaoProposals<T>>::insert((dao_id, proposal_id), proposal.clone());
            }

            <DaoProposalsVotes<T>>::insert((dao_id, proposal_id, dao_proposal_votes_count), &voter);
            <DaoProposalsVotesCount<T>>::insert((dao_id, proposal_id), new_dao_proposals_votes_count);
            <DaoProposalsVotesIndex<T>>::insert((dao_id, proposal_id, voter.clone()), dao_proposal_votes_count);

            Self::deposit_event(RawEvent::NewVote(dao_id, proposal_id, voter, vote));

            match (proposal_is_accepted, proposal_is_rejected) {
                (true, _) => Self::deposit_event(RawEvent::ProposalIsAccepted(dao_id, proposal_id)),
                (_, true) => Self::deposit_event(RawEvent::ProposalIsRejected(dao_id, proposal_id)),
                (_, _) => ()
            }

            Ok(())
        }

        pub fn put_money(origin, dao_id: DaoId, value: T::Balance) -> Result {
            let depositor = ensure_signed(origin)?;

            ensure!(<DaoMembers<T>>::exists((dao_id, depositor.clone())), "You are not a member of this DAO");
            ensure!(<balances::FreeBalance<T>>::get(depositor.clone()) >= value, "You dont have enough balance to make this deposit");
            
            let dao_address = <Address<T>>::get(dao_id);
            let dao_dalance = <balances::FreeBalance<T>>::get(dao_address.clone());

            <balances::Module<T> as Currency<_>>::transfer(&depositor, &dao_address, value)?;
            
            Self::deposit_event(RawEvent::Deposit(depositor, dao_address, value));

            Ok(())
        }

        fn on_finalize() {
            let block_number = <system::Module<T>>::block_number();
            Self::open_dao_proposals(block_number)
                .iter()
                .for_each(|&proposal_id| {
                    let dao_id = <DaoProposalsIndex<T>>::get(proposal_id);
                    let proposal = <DaoProposals<T>>::get((dao_id, proposal_id));

                    if proposal.open {
                        Self::close_proposal(dao_id, proposal_id, proposal);

                        Self::deposit_event(RawEvent::ProposalIsExpired(dao_id, proposal_id));
                    }
                });

            <OpenDaoProposals<T>>::remove(block_number);
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
        Balance = <T as balances::Trait>::Balance,
        AccountId = <T as system::Trait>::AccountId,
        BlockNumber = <T as system::Trait>::BlockNumber,
    {
        Deposit(AccountId, AccountId, Balance),
        DaoCreated(AccountId, AccountId, Vec<u8>),
        NewVote(DaoId, ProposalId, AccountId, bool),
        ProposalIsAccepted(DaoId, ProposalId),
        ProposalIsExpired(DaoId, ProposalId),
        ProposalIsRejected(DaoId, ProposalId),
        ProposeToAddMember(DaoId, AccountId, BlockNumber),
        ProposeToRemoveMember(DaoId, AccountId, BlockNumber),
    }
);

impl<T: Trait> Module<T> {
    fn validate_name(name: &Vec<u8>) -> Result {
        if name.len() < 10 {
            return Err("the DAO name is very short");
        }
        if name.len() > 255 {
            return Err("the DAO name is very long");
        }

        let is_valid_char = |&c| {
            (c >= 97 && c <= 122) || // 'a' - 'z'
            (c >= 65 && c <= 90) ||  // 'A' - 'Z'
            (c >= 48 && c <= 57) ||  // '0' - '9'
            c == 45 || c == 95 // '-', '_'
        };
        if !(name.iter().all(is_valid_char)) {
            return Err("the DAO name has invalid chars");
        }

        Ok(())
    }

    fn validate_description(description: &Vec<u8>) -> Result {
        if description.len() < 10 {
            return Err("the DAO description is very short");
        }
        if description.len() > 4096 {
            return Err("the DAO description is very long");
        }

        Ok(())
    }

    fn add_member(dao_id: DaoId, member: T::AccountId) -> Result {
        ensure!(
            <MembersCount<T>>::get(dao_id) + 1 <= Self::maximum_number_of_members(),
            "Maximum number of members for this DAO is reached"
        );

        let members_count = <MembersCount<T>>::get(dao_id);
        let new_members_count = members_count
            .checked_add(1)
            .ok_or("Overflow adding a member to DAO")?;

        <Members<T>>::insert((dao_id, members_count), &member);
        <MembersCount<T>>::insert(dao_id, new_members_count);
        <DaoMembers<T>>::insert((dao_id, member), members_count);

        Ok(())
    }

    fn remove_memeber(dao_id: DaoId, member: T::AccountId) -> Result {
        let members_count = <MembersCount<T>>::get(dao_id);

        let new_members_count = members_count
            .checked_sub(1)
            .ok_or("Underflow removing a member from DAO")?;

        let member_id = <DaoMembers<T>>::get((dao_id, member.clone()));

        if member_id != members_count {
            let latest_member = <Members<T>>::get((dao_id, members_count));
            <Members<T>>::insert((dao_id, member_id), &latest_member);
            <DaoMembers<T>>::insert((dao_id, latest_member), member_id);
        }
        <Members<T>>::remove((dao_id, members_count));
        <MembersCount<T>>::insert(dao_id, new_members_count);
        <DaoMembers<T>>::remove((dao_id, member));

        Ok(())
    }

    fn close_proposal(
        dao_id: DaoId,
        proposal_id: ProposalId,
        mut proposal: Proposal<DaoId, T::AccountId, T::BlockNumber, MemberId>,
    ) {
        proposal.open = false;
        let proposal_hash = <OpenDaoProposalsHashesIndex<T>>::get(proposal_id);

        <DaoProposals<T>>::insert((dao_id, proposal_id), proposal);
        <OpenDaoProposalsHashes<T>>::remove(proposal_hash);
        <OpenDaoProposalsHashesIndex<T>>::remove(proposal_id);
    }

    fn votes_are_enough(votes: MemberId, maximum_votes: MemberId) -> bool {
        votes as f64 / maximum_votes as f64 >= 0.51
    }

    fn execute_proposal(
        proposal: &Proposal<DaoId, T::AccountId, T::BlockNumber, MemberId>,
    ) -> Result {
        match &proposal.action {
            Action::AddMember(member) => Self::add_member(proposal.dao_id, member.clone()),
            Action::RemoveMember(member) => Self::remove_memeber(proposal.dao_id, member.clone()),
            Action::EmptyAction => Ok(()),
        }
    }
}

/// tests for this module
#[cfg(test)]
mod tests {
    use super::*;

    use primitives::{Blake2Hasher, H256};
    use runtime_io::with_externalities;
    use runtime_primitives::{
        testing::{Digest, DigestItem, Header},
        traits::{BlakeTwo256, IdentityLookup},
        BuildStorage,
    };
    use support::{assert_noop, assert_ok, impl_outer_origin};

    impl_outer_origin! {
        pub enum Origin for Test {}
    }

    // For testing the module, we construct most of a mock runtime. This means
    // first constructing a configuration type (`Test`) which `impl`s each of the
    // configuration traits of modules we want to use.
    #[derive(Clone, Eq, PartialEq)]
    pub struct Test;
    impl system::Trait for Test {
        type Origin = Origin;
        type Index = u64;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type Digest = Digest;
        type AccountId = u64;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = ();
        type Log = DigestItem;
    }
    impl balances::Trait for Test {
        type Balance = u128;
        type OnFreeBalanceZero = ();
        type OnNewAccount = ();
        type TransactionPayment = ();
        type TransferPayment = ();
        type DustRemoval = ();
        type Event = ();
    }
    impl timestamp::Trait for Test {
        type Moment = u64;
        type OnTimestampSet = ();
    }
    impl Trait for Test {
        type Event = ();
    }
    type DaoModule = Module<Test>;

    const DAO_NAME: &[u8; 10] = b"Name-1234_";
    const DAO_NAME2: &[u8; 10] = b"Name-5678_";
    const DAO_DESC: &[u8; 10] = b"Desc-1234_";
    const USER: u64 = 1;
    const USER2: u64 = 2;
    const USER3: u64 = 3;
    const USER4: u64 = 4;
    const USER5: u64 = 5;
    const DAO: u64 = 11;
    const DAO2: u64 = 12;

    // This function basically just builds a genesis storage key/value store according to
    // our desired mockup.
    fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
        system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap()
            .0
            .into()
    }

    #[test]
    fn create_dao_should_work() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const MEMBER_ID: MemberId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_eq!(DaoModule::members_count(DAO_ID), 0);
            assert_ne!(DaoModule::members((DAO_ID, MEMBER_ID)), USER);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_eq!(DaoModule::members_count(DAO_ID), 1);
            assert_eq!(DaoModule::members((DAO_ID, MEMBER_ID)), USER);
            assert_eq!(DaoModule::dao_members((DAO_ID, USER)), MEMBER_ID);
        })
    }

    #[test]
    fn dao_name_is_very_short() {
        with_externalities(&mut new_test_ext(), || {
            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO,
                    DAO_NAME.to_vec().drain(1..).collect(),
                    DAO_DESC.to_vec()
                ),
                "the DAO name is very short"
            );
            assert_eq!(DaoModule::daos_count(), 0);
        })
    }

    #[test]
    fn dao_name_has_invalid_chars() {
        with_externalities(&mut new_test_ext(), || {
            const ASCII_CODE_OF_PLUS: u8 = 43;

            let mut name = DAO_NAME.to_vec();
            name.push(ASCII_CODE_OF_PLUS);

            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::create(Origin::signed(USER), DAO, name, DAO_DESC.to_vec()),
                "the DAO name has invalid chars"
            );
            assert_eq!(DaoModule::daos_count(), 0);
        })
    }

    #[test]
    fn dao_name_is_very_long() {
        with_externalities(&mut new_test_ext(), || {
            const ASCII_CODE_OF_A: u8 = 97;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO,
                    [ASCII_CODE_OF_A; 256].to_vec(),
                    DAO_DESC.to_vec()
                ),
                "the DAO name is very long"
            );
            assert_eq!(DaoModule::daos_count(), 0);
        })
    }

    #[test]
    fn dao_address_already_busy() {
        with_externalities(&mut new_test_ext(), || {
            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO,
                    DAO_NAME.to_vec(),
                    DAO_DESC.to_vec()
                ),
                "This DAO address already busy"
            );
            assert_eq!(DaoModule::daos_count(), 1);
        })
    }

    #[test]
    fn dao_name_already_exists() {
        with_externalities(&mut new_test_ext(), || {
            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO2,
                    DAO_NAME.to_vec(),
                    DAO_DESC.to_vec()
                ),
                "This DAO name already exists"
            );
            assert_eq!(DaoModule::daos_count(), 1);
        })
    }

    #[test]
    fn dao_description_is_very_short() {
        with_externalities(&mut new_test_ext(), || {
            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO,
                    DAO_NAME.to_vec(),
                    DAO_DESC.to_vec().drain(1..).collect()
                ),
                "the DAO description is very short"
            );
            assert_eq!(DaoModule::daos_count(), 0);
        })
    }

    #[test]
    fn dao_description_is_very_long() {
        with_externalities(&mut new_test_ext(), || {
            const ASCII_CODE_OF_A: u8 = 97;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO,
                    DAO_NAME.to_vec().to_vec(),
                    [ASCII_CODE_OF_A; 4097].to_vec()
                ),
                "the DAO description is very long"
            );
            assert_eq!(DaoModule::daos_count(), 0);
        })
    }

    #[test]
    fn dao_address_is_already_busy() {
        with_externalities(&mut new_test_ext(), || {
            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO,
                    DAO_NAME.to_vec().to_vec(),
                    DAO_DESC.to_vec()
                ),
                "This DAO address already busy"
            );
        })
    }

    #[test]
    fn dao_name_is_already_exists() {
        with_externalities(&mut new_test_ext(), || {
            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_noop!(
                DaoModule::create(
                    Origin::signed(USER),
                    DAO2,
                    DAO_NAME.to_vec().to_vec(),
                    DAO_DESC.to_vec()
                ),
                "This DAO name already exists"
            );
        })
    }

    #[test]
    fn propose_to_add_member_should_work() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER2),
                DAO_ID
            ));
        })
    }

    #[test]
    fn propose_to_add_member_case_this_dao_not_exists() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::propose_to_add_member(Origin::signed(USER), DAO_ID),
                "This DAO not exists"
            );
        })
    }

    #[test]
    fn propose_to_add_member_case_you_already_are_a_member_of_this_dao() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_noop!(
                DaoModule::propose_to_add_member(Origin::signed(USER), DAO_ID),
                "You already are a member of this DAO"
            );
        })
    }

    #[test]
    fn propose_to_add_member_case_dao_can_not_be_a_member_of_other_dao() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const DAO_ID2: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO2,
                DAO_NAME2.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 2);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_eq!(DaoModule::members((DAO_ID2, 0)), USER);
            assert_noop!(
                DaoModule::propose_to_add_member(Origin::signed(DAO), DAO_ID2),
                "A DAO can not be a member of other DAO"
            );
        })
    }

    #[test]
    fn propose_to_add_member_case_maximum_number_of_members_is_reached() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::add_member(DAO_ID, USER3));
            assert_ok!(DaoModule::add_member(DAO_ID, USER4));
            assert_eq!(DaoModule::members_count(DAO_ID), 4);
            assert_noop!(
                DaoModule::propose_to_add_member(Origin::signed(USER5), DAO_ID),
                "Maximum number of members for this DAO is reached"
            );
        })
    }

    #[test]
    fn propose_to_add_member_case_this_proposal_already_open() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            assert_noop!(
                DaoModule::propose_to_add_member(Origin::signed(USER2), DAO_ID),
                "This proposal already open"
            );
        })
    }

    #[test]
    fn propose_to_add_member_case_maximum_number_of_open_proposals_is_reached() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER3),
                DAO_ID
            ));
            assert_noop!(
                DaoModule::propose_to_add_member(Origin::signed(USER4), DAO_ID),
                "Maximum number of open proposals is reached for the target block, try later"
            );
        })
    }

    #[test]
    fn propose_to_remove_member_should_work() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);

            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::propose_to_remove_member(
                Origin::signed(USER2),
                DAO_ID
            ));
        })
    }

    #[test]
    fn propose_to_remove_member_case_this_dao_not_exists() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_noop!(
                DaoModule::propose_to_remove_member(Origin::signed(USER), DAO_ID),
                "This DAO not exists"
            );
        })
    }

    #[test]
    fn propose_to_remove_member_case_you_already_are_not_member() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_noop!(
                DaoModule::propose_to_remove_member(Origin::signed(USER2), DAO_ID),
                "You already are not a member of this DAO"
            );
        })
    }

    #[test]
    fn propose_to_remove_member_case_you_are_the_latest_member() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_noop!(
                DaoModule::propose_to_remove_member(Origin::signed(USER), DAO_ID),
                "You are the latest member of this DAO"
            );
        })
    }

    #[test]
    fn propose_to_remove_member_case_this_proposal_already_open() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::propose_to_remove_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            assert_noop!(
                DaoModule::propose_to_remove_member(Origin::signed(USER2), DAO_ID),
                "This proposal already open"
            );
        })
    }

    #[test]
    fn propose_to_remove_member_case_maximum_number_of_open_proposals_is_reached() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::add_member(DAO_ID, USER3));
            assert_ok!(DaoModule::add_member(DAO_ID, USER4));
            assert_ok!(DaoModule::propose_to_remove_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            assert_ok!(DaoModule::propose_to_remove_member(
                Origin::signed(USER3),
                DAO_ID
            ));
            assert_noop!(
                DaoModule::propose_to_remove_member(Origin::signed(USER4), DAO_ID),
                "Maximum number of open proposals is reached for the target block, try later"
            );
        })
    }

    #[test]
    fn vote_should_work() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            assert_eq!(
                DaoModule::dao_proposals_votes_count((DAO_ID, PROPOSAL_ID)),
                0
            );
            assert_ok!(DaoModule::vote(
                Origin::signed(USER),
                DAO_ID,
                PROPOSAL_ID,
                YES
            ));
            assert_eq!(
                DaoModule::dao_proposals_votes_count((DAO_ID, PROPOSAL_ID)),
                1
            );
        })
    }

    #[test]
    fn vote_should_work_early_ending_of_voting_case_all_yes() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::add_member(DAO_ID, USER3));

            assert_eq!(DaoModule::members_count(DAO_ID), 3);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_eq!(DaoModule::members((DAO_ID, 1)), USER2);
            assert_eq!(DaoModule::members((DAO_ID, 2)), USER3);
            assert_ne!(DaoModule::members((DAO_ID, 3)), USER4);

            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER4),
                DAO_ID
            ));

            assert_ok!(DaoModule::vote(
                Origin::signed(USER),
                DAO_ID,
                PROPOSAL_ID,
                YES
            ));
            assert_eq!(DaoModule::members_count(DAO_ID), 3);

            assert_ok!(DaoModule::vote(
                Origin::signed(USER2),
                DAO_ID,
                PROPOSAL_ID,
                YES
            ));
            assert_eq!(DaoModule::members_count(DAO_ID), 4);

            assert_noop!(
                DaoModule::vote(Origin::signed(USER3), DAO_ID, PROPOSAL_ID, YES),
                "This proposal is not open"
            );

            assert_eq!(DaoModule::members_count(DAO_ID), 4);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_eq!(DaoModule::members((DAO_ID, 1)), USER2);
            assert_eq!(DaoModule::members((DAO_ID, 2)), USER3);
            assert_eq!(DaoModule::members((DAO_ID, 3)), USER4);
        })
    }

    #[test]
    fn vote_should_work_early_ending_of_voting_case_all_no() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const NO: bool = false;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::add_member(DAO_ID, USER3));

            assert_eq!(DaoModule::members_count(DAO_ID), 3);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_eq!(DaoModule::members((DAO_ID, 1)), USER2);
            assert_eq!(DaoModule::members((DAO_ID, 2)), USER3);
            assert_ne!(DaoModule::members((DAO_ID, 3)), USER4);

            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER4),
                DAO_ID
            ));

            assert_ok!(DaoModule::vote(
                Origin::signed(USER),
                DAO_ID,
                PROPOSAL_ID,
                NO
            ));
            assert_eq!(DaoModule::members_count(DAO_ID), 3);

            assert_ok!(DaoModule::vote(
                Origin::signed(USER2),
                DAO_ID,
                PROPOSAL_ID,
                NO
            ));
            assert_eq!(DaoModule::members_count(DAO_ID), 3);

            assert_noop!(
                DaoModule::vote(Origin::signed(USER3), DAO_ID, PROPOSAL_ID, NO),
                "This proposal is not open"
            );

            assert_eq!(DaoModule::members_count(DAO_ID), 3);
            assert_eq!(DaoModule::members((DAO_ID, 0)), USER);
            assert_eq!(DaoModule::members((DAO_ID, 1)), USER2);
            assert_eq!(DaoModule::members((DAO_ID, 2)), USER3);
            assert_ne!(DaoModule::members((DAO_ID, 3)), USER4);
        })
    }

    #[test]
    fn vote_case_you_are_not_member_of_this_dao() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_noop!(
                DaoModule::vote(Origin::signed(USER2), DAO_ID, PROPOSAL_ID, YES),
                "You are not a member of this DAO"
            );
        })
    }

    #[test]
    fn vote_case_this_proposal_not_exists() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_noop!(
                DaoModule::vote(Origin::signed(USER), DAO_ID, PROPOSAL_ID, YES),
                "This proposal not exists"
            );
        })
    }

    #[test]
    fn vote_case_you_voted_already() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            assert_ok!(DaoModule::vote(
                Origin::signed(USER),
                DAO_ID,
                PROPOSAL_ID,
                YES
            ));
            assert_noop!(
                DaoModule::vote(Origin::signed(USER), DAO_ID, PROPOSAL_ID, YES),
                "You voted already"
            );
        })
    }

    #[test]
    fn vote_case_this_proposal_is_not_open() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER2),
                DAO_ID
            ));
            DaoModule::close_proposal(
                DAO_ID,
                PROPOSAL_ID,
                DaoModule::dao_proposals((DAO_ID, PROPOSAL_ID)),
            );
            assert_noop!(
                DaoModule::vote(Origin::signed(USER), DAO_ID, PROPOSAL_ID, YES),
                "This proposal is not open"
            );
        })
    }

    #[test]
    fn vote_case_maximum_number_of_members_is_reached() {
        with_externalities(&mut new_test_ext(), || {
            const DAO_ID: DaoId = 0;
            const PROPOSAL_ID: ProposalId = 0;
            const YES: bool = true;

            assert_eq!(DaoModule::daos_count(), 0);
            assert_ok!(DaoModule::create(
                Origin::signed(USER),
                DAO,
                DAO_NAME.to_vec(),
                DAO_DESC.to_vec()
            ));
            assert_eq!(DaoModule::daos_count(), 1);

            assert_ok!(DaoModule::add_member(DAO_ID, USER2));
            assert_ok!(DaoModule::add_member(DAO_ID, USER3));
            assert_eq!(DaoModule::members_count(DAO_ID), 3);
            assert_ok!(DaoModule::propose_to_add_member(
                Origin::signed(USER4),
                DAO_ID
            ));
            assert_ok!(DaoModule::add_member(DAO_ID, USER5));
            assert_eq!(DaoModule::members_count(DAO_ID), 4);
            assert_ok!(DaoModule::vote(
                Origin::signed(USER),
                DAO_ID,
                PROPOSAL_ID,
                YES
            ));
            assert_ok!(DaoModule::vote(
                Origin::signed(USER2),
                DAO_ID,
                PROPOSAL_ID,
                YES
            ));
            assert_noop!(
                DaoModule::vote(Origin::signed(USER3), DAO_ID, PROPOSAL_ID, YES),
                "Maximum number of members for this DAO is reached"
            );
        })
    }
}