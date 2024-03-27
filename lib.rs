#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[openbrush::implementation(PSP34, PSP34Mintable)]
#[openbrush::contract]
pub mod videown {
    use openbrush::{
        contracts::psp34::{psp34, BalancesManager, PSP34Error},
        storage::Mapping,
        traits::{Storage, String},
    };

    #[ink(storage)]
    #[derive(Default, Storage)]
    pub struct Videown {
        #[storage_field]
        psp34: psp34::Data,
        /// Current asks: tokenId -> price
        asks: Mapping<Id, Balance>,
    }

    /// Event emitted when a token transfer occurs.
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        #[ink(topic)]
        id: Id,
    }

    /// Event emitted when a token approve occurs.
    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        #[ink(topic)]
        id: Option<Id>,
        approved: bool,
    }

    /// Event emitted when a token trade occurs.
    #[ink(event)]
    pub struct Trade {
        #[ink(topic)]
        seller: AccountId,
        #[ink(topic)]
        buyer: AccountId,
        #[ink(topic)]
        id: Id,
        price: Balance,
    }

    #[default_impl(BalancesManager)]
    fn _increase_balance(&mut self, owner: &Owner, id: &Id, increase_supply: bool) {}

    #[default_impl(BalancesManager)]
    fn _decrease_balance(&mut self, owner: &Owner, id: &Id, decrease_supply: bool) {}

    #[overrider(psp34::Internal)]
    fn _before_token_transfer(
        &mut self,
        _from: Option<&AccountId>,
        _to: Option<&AccountId>,
        id: &Id,
    ) -> Result<(), PSP34Error> {
        // The token must not be on sale before the token is transferred
        if self.asks.get(id).is_some() {
            return Err(Error::TransferTokenInSale.into());
        }
        Ok(())
    }

    impl Videown {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self::default()
        }

        #[ink(message)]
        pub fn ask(&mut self, id: Id, price: Balance) -> Result<(), Error> {
            let owner = psp34::Internal::_check_token_exists(self, &id)?;
            let caller = self.env().caller();
            if owner != caller {
                return Err(Error::NotTokenOwner);
            }

            self.asks.insert(&id, &price);

            Ok(())
        }

        #[ink(message, payable)]
        pub fn buy(&mut self, id: Id) -> Result<(), Error> {
            let owner = psp34::Internal::_check_token_exists(self, &id)?;
            let caller = self.env().caller();
            if owner == caller {
                return Err(Error::SelfBuy);
            }

            let transferred = self.env().transferred_value();
            let price = self.asks.get(&id).ok_or(Error::NotInSale)?;
            if transferred != price {
                return Err(Error::NotMatchPrice);
            }

            // transfer native token
            self.env()
                .transfer(owner, price)
                .map_err(|_| Error::NativeTransfer)?;

            self.asks.remove(&id);

            // transfer nft token
            psp34::Internal::_before_token_transfer(self, Some(&owner), Some(&caller), &id)?;
            self._remove_operator_approvals(&owner, &caller, &Some(&id));
            self._decrease_balance(&owner, &id, false);
            self._remove_token_owner(&id);
            self._increase_balance(&caller, &id, false);
            self._insert_token_owner(&id, &caller);
            psp34::Internal::_after_token_transfer(self, Some(&owner), Some(&caller), &id)?;

            psp34::Internal::_emit_transfer_event(self, Some(owner), Some(caller), id.clone());
            self.env().emit_event(Trade {
                seller: owner,
                buyer: caller,
                id,
                price,
            });

            Ok(())
        }

        #[ink(message)]
        pub fn cancel(&mut self, id: Id) -> Result<(), Error> {
            let owner = psp34::Internal::_check_token_exists(self, &id)?;
            let caller = self.env().caller();
            if owner != caller {
                return Err(Error::NotTokenOwner);
            }
            if self.asks.get(&id).is_none() {
                return Err(Error::NotInSale);
            }

            self.asks.remove(&id);

            Ok(())
        }

        #[ink(message)]
        pub fn price(&self, id: Id) -> Option<Balance> {
            self.asks.get(&id)
        }
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotTokenOwner,
        SelfBuy,
        NotInSale,
        NotMatchPrice,
        TransferTokenInSale,
        Psp34(PSP34Error),
        NativeTransfer,
    }

    impl From<PSP34Error> for Error {
        fn from(err: PSP34Error) -> Self {
            Self::Psp34(err)
        }
    }

    impl From<Error> for PSP34Error {
        fn from(err: Error) -> Self {
            use Error::*;
            match err {
                TransferTokenInSale => PSP34Error::Custom(String::from("TransferTokenInSale")),
                _ => PSP34Error::Custom(String::from("Undefined for PSP34")),
            }
        }
    }
}
