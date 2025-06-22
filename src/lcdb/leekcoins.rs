use crate::{
    lcdb::{self, DBResult},
    models,
};

/// Adds ``amount`` to ``user``'s LeekCoin balance.
///
/// Returns their new balance.
pub fn add_balance(user: &models::User, amount: usize) -> DBResult<usize> {
    log::trace!(
        "[add_balance] Adding balance of {amount} to {}",
        user.username
    );
    modify_balance(user, amount, true).inspect(|balance| {
        log::info!(
            "[add_balance] {}'s new balance is {balance} (+{amount})",
            user.username
        );
    })
}

/// Subtract ``amount`` from ``user``'s LeekCoin balance.
///
/// Returns their new balance.
pub fn sub_balance(user: &models::User, amount: usize) -> DBResult<usize> {
    log::trace!(
        "[sub_balance] Subtracting balance of {amount} to {}",
        user.username
    );
    modify_balance(user, amount, false).inspect(|balance| {
        log::info!(
            "[sub_balance] {}'s new balance is {balance} (-{amount})",
            user.username
        );
    })
}

/// Queries the LeekCoin balance for ``user``.
///
/// Returns their balance.
pub fn query_balance(user: &models::User) -> DBResult<usize> {
    log::trace!("[query_balance] Querying balance for {}", user.username);

    let connection = lcdb::connect()?;
    let query_params = rusqlite::named_params! { ":username": user.username };

    connection
        .prepare("SELECT * FROM LeekCoins WHERE (username = :username)")?
        .query_row(query_params, |row| row.get("balance"))
}

/// Modify the balance of a ``user`` by an ``amount``, either incrementing or decrementing.
///
/// Returns their new balance.
fn modify_balance(user: &models::User, amount: usize, increment: bool) -> DBResult<usize> {
    let connection = lcdb::connect()?;

    let query_params = rusqlite::named_params! {
            ":username": user.username,
            ":amount": amount,
            ":op": if increment {"+"} else {"-"},
    };

    connection
        .prepare(
            "INSERT OR IGNORE INTO LeekCoins (username, balance) VALUES (:username, 0);
             UPDATE LeekCoins SET balance = (balance :op :amount) WHERE (username = :username);
             SELECT * FROM LeekCoins WHERE (username = :username);",
        )?
        .query_row(query_params, |row| row.get("balance"))
}
