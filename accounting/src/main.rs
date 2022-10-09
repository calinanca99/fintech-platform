use std::{io, process};

use accounting::{accounts::Accounts, tx::Tx};

fn read_from_stdin(label: &str) -> String {
    println!("{label}");

    let mut buffer = String::new();
    io::stdin()
        .read_line(&mut buffer)
        .expect("cannot read line");

    println!("-------------");

    buffer.trim().to_string()
}

fn main() {
    let mut ledger = Accounts::new();
    let mut tx_log = vec![];
    loop {
        let user_input = read_from_stdin("Enter a command: ");

        match user_input.as_str() {
            "deposit" => handle_deposit(&mut ledger, &mut tx_log),
            "withdraw" => handle_withdraw(&mut ledger, &mut tx_log),
            "send" => handle_send(&mut ledger, &mut tx_log),
            "print" => {
                println!("{ledger:#?}");
            }
            "quit" => process::exit(1),
            _ => println!("Command '{user_input}' not found."),
        }
    }
}

fn handle_deposit(ledger: &mut Accounts, tx_log: &mut Vec<Tx>) {
    let signer = read_from_stdin("Enter signer: ");
    let amount = read_from_stdin("Enter amount: ").parse::<u64>();

    match amount {
        Ok(amount) => match ledger.deposit(signer.as_str(), amount) {
            Ok(tx) => {
                tx_log.push(tx);
            }
            Err(accounting_error) => println!("{accounting_error:?}"),
        },
        Err(e) => println!("{e}"),
    }
}

fn handle_withdraw(ledger: &mut Accounts, tx_log: &mut Vec<Tx>) {
    let signer = read_from_stdin("Enter signer: ");
    let amount = read_from_stdin("Enter amount: ").parse::<u64>();

    match amount {
        Ok(amount) => match ledger.withdraw(signer.as_str(), amount) {
            Ok(tx) => {
                tx_log.push(tx);
            }
            Err(accounting_error) => println!("{accounting_error:?}"),
        },
        Err(e) => println!("{e}"),
    }
}

fn handle_send(ledger: &mut Accounts, tx_log: &mut Vec<Tx>) {
    let sender = read_from_stdin("Enter sender: ");
    let recipient = read_from_stdin("Enter recipient: ");
    let amount = read_from_stdin("Enter amount: ").parse::<u64>();

    match amount {
        Ok(amount) => match ledger.send(sender.as_str(), recipient.as_str(), amount) {
            Ok((withdraw_tx, deposit_tx)) => {
                tx_log.push(withdraw_tx);
                tx_log.push(deposit_tx);
            }
            Err(accounting_error) => println!("{accounting_error:?}"),
        },
        Err(e) => println!("{e}"),
    }
}
