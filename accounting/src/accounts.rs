use std::collections::HashMap;

use crate::{errors::AccountingError, tx::Tx};

/// A type for managing accounts and their current currency balance
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts {
    accounts: HashMap<String, u64>,
}

impl Accounts {
    /// Returns an empty instance of the [`Accounts`] type
    pub fn new() -> Self {
        Accounts {
            accounts: Default::default(),
        }
    }

    /// Either deposits the `amount` provided into the `signer` account or adds the amount to the existing account.
    ///
    /// # Errors
    /// - attempted overflow
    pub fn deposit(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        if let Some(account) = self.accounts.get_mut(signer) {
            (*account)
                .checked_add(amount)
                .map(|new_amount| *account = new_amount)
                .ok_or_else(|| AccountingError::AccountOverFunded(signer.to_string(), amount))
                // Using map() here is an easy way to only manipulate the non-error result
                .map(|_| Tx::Deposit {
                    account: signer.to_string(),
                    amount,
                })
        } else {
            self.accounts.insert(signer.to_string(), amount);
            Ok(Tx::Deposit {
                account: signer.to_string(),
                amount,
            })
        }
    }

    /// Withdraws the `amount` from the `signer` account.
    ///
    /// # Errors
    /// - insufficient funds
    /// - inexistent account
    pub fn withdraw(&mut self, signer: &str, amount: u64) -> Result<Tx, AccountingError> {
        if let Some(account) = self.accounts.get_mut(signer) {
            (*account)
                .checked_sub(amount)
                .map(|new_amount| *account = new_amount)
                .ok_or_else(|| AccountingError::AccountUnderFunded(signer.to_string(), amount))
                .map(|_| Tx::Withdraw {
                    account: signer.to_string(),
                    amount,
                })
        } else {
            Err(AccountingError::AccountNotFound(signer.to_string()))
        }
    }

    /// Withdraws the amount from the sender account and deposits it in the recipient account.
    ///
    /// # Errors
    /// - inexistent `sender` account
    /// - `sender` has insufficient funds
    /// - deposit can cause overflow for `recipient`
    pub fn send(
        &mut self,
        sender: &str,
        recipient: &str,
        amount: u64,
    ) -> Result<(Tx, Tx), AccountingError> {
        Ok((
            self.withdraw(sender, amount)?,
            self.deposit(recipient, amount)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{errors::AccountingError, tx::Tx};

    use super::Accounts;

    #[test]
    fn when_a_new_user_makes_a_deposit_it_is_added_in_accounts() {
        // Arrange
        let mut accounts = Accounts::new();
        let signer = "client_1";
        let deposit = 100;

        // Act
        let sut = accounts.deposit(signer, deposit);

        // Assert
        assert_eq!(
            Tx::Deposit {
                account: signer.to_string(),
                amount: deposit
            },
            sut.unwrap()
        );
        assert_eq!(accounts.accounts[signer], deposit);
    }

    #[test]
    fn when_an_existent_user_makes_a_transaction_the_amount_is_correctly_updated() {
        // Arrange
        let mut accounts = Accounts::new();
        let signer = "client_1";
        let first_deposit = 100;
        let second_deposit = 150;

        accounts
            .deposit(signer, first_deposit)
            .expect("first deposit failed");

        // Act
        let sut = accounts.deposit(signer, second_deposit);

        // Assert
        assert_eq!(
            Tx::Deposit {
                account: signer.to_string(),
                amount: second_deposit
            },
            sut.unwrap()
        );
        assert_eq!(accounts.accounts[signer], first_deposit + second_deposit);
    }

    #[test]
    fn errors_when_a_deposit_causes_an_overflow() {
        // Arrange
        let mut accounts = Accounts::new();
        let signer = "client_1";
        let first_deposit = 100;
        let second_deposit = u64::MAX;

        accounts
            .deposit(signer, first_deposit)
            .expect("deposit failed");

        // Act
        let previous_accounts = accounts.clone();
        let sut = accounts.deposit(signer, second_deposit);

        // Assert
        assert_eq!(
            Err(AccountingError::AccountOverFunded(
                signer.to_string(),
                second_deposit
            )),
            sut
        );
        assert_eq!(previous_accounts, accounts);
    }

    #[test]
    fn withdrawing_correctly_updates_the_account_on_the_happy_path() {
        // Arrange
        let mut accounts = Accounts::new();
        let signer = "client_1";
        let deposit = 100;
        let withdraw = 50;

        accounts.deposit(signer, deposit).expect("deposit failed");

        // Act
        let sut = accounts.withdraw(signer, withdraw);

        // Assert
        assert_eq!(
            Tx::Withdraw {
                account: signer.to_string(),
                amount: withdraw
            },
            sut.unwrap()
        );
        assert_eq!(accounts.accounts[signer], deposit - withdraw);
    }

    #[test]
    fn errors_when_withdrawing_from_a_nonexistent_account() {
        // Arrange
        let mut accounts = Accounts::new();
        let signer = "client_1";
        let withdraw = 100;

        // Act
        let previous_accounts = accounts.clone();
        let sut = accounts.withdraw(signer, withdraw);

        // Assert
        assert_eq!(
            Err(AccountingError::AccountNotFound(signer.to_string())),
            sut
        );
        assert_eq!(previous_accounts, accounts);
    }

    #[test]
    fn errors_when_withdrawing_more_than_is_available() {
        // Arrange
        let mut accounts = Accounts::new();
        let signer = "client_1";
        let deposit = 100;
        let withdraw = 200;

        accounts.deposit(signer, deposit).expect("deposit failed");

        // Act
        let previous_accounts = accounts.clone();
        let sut = accounts.withdraw(signer, withdraw);

        // Assert
        assert_eq!(
            Err(AccountingError::AccountUnderFunded(
                signer.to_string(),
                withdraw
            )),
            sut
        );
        assert_eq!(previous_accounts, accounts);
    }

    #[test]
    fn sending_money_correctly_updates_both_accounts_on_the_happy_path() {
        // Arrange
        let mut accounts = Accounts::new();

        let sender = "client_1";
        let sender_deposit = 100;
        accounts
            .deposit(sender, sender_deposit)
            .expect("deposit failed");

        let recipient = "client_2";

        let transferred_amount = 50;

        // Act
        let sut = accounts.send(sender, recipient, transferred_amount);

        // Assert
        assert_eq!(
            (
                Tx::Withdraw {
                    account: sender.to_string(),
                    amount: transferred_amount
                },
                Tx::Deposit {
                    account: recipient.to_string(),
                    amount: transferred_amount
                }
            ),
            sut.unwrap()
        );
        assert_eq!(
            accounts.accounts[sender],
            sender_deposit - transferred_amount
        );
        assert_eq!(accounts.accounts[recipient], transferred_amount);
    }
}
