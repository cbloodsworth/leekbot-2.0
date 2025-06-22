use crate::{lcdb::{connect, DBResult}, models};


/////*============== PROBLEM QUERIES ==============*/
/// Inserts the problem into Problems, or does nothing if it already is there.
/// Returns `true` if it was newly added, false otherwise.
pub fn insert_problem(problem: &models::Problem) -> DBResult<bool> {
    let connection = connect()?;

    log::trace!(
        "[insert_problem] Inserting problem {} into Problems...",
        problem.title
    );

    let query_params = rusqlite::named_params! {
            ":problem_name": problem.title,
            ":problem_link": format!("https://leetcode.com/problems/{}", problem.url),
            ":difficulty":   problem.difficulty
    };

    connection
        .prepare(
            "INSERT INTO Problems ( problem_name,  problem_link,  difficulty)
         VALUES                         (:problem_name, :problem_link, :difficulty)",
        )?
        .execute(query_params)
        .map_or_else(crate::lcdb::swallow_constraint_violation, |_| Ok(true))
}
