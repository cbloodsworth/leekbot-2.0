use crate::lcapi;
use crate::lcdb;
use crate::models::AnnouncementPreferences;

use anyhow::{Context, Result, anyhow};
use itertools::Itertools;
use serenity::model::channel::Message;

const MAX_CMD_LENGTH: usize = 12;

pub struct Commands;
impl Commands {
    pub async fn run_command(ctx: &serenity::client::Context, msg: &Message) -> Result<String> {
        let react_ok = async || -> Result<String> {
            msg.react(
                &ctx.http,
                serenity::all::ReactionType::Unicode(String::from("✅")),
            )
            .await?;

            Ok(String::from(""))
        };

        // Split the message's content (on whitespace) into:
        // - The command (first token)
        // - Its parameters (all tokens afterwards)

        // Skip the first letter for the command: it's the call token
        let input = String::from(&msg.content[1..]); 
        let split_tokens = input.split_whitespace().collect::<Vec<_>>();
        let (&[command], parameters) = split_tokens.split_at(1) else {
            return Err(anyhow!("easd"));
        };

        // Execute the command
        let result: String = match command {
            "audit" => {
                let username = parameters
                    .first()
                    .context("Expected username for audit, got none.")?
                    .to_string();

                let user = lcapi::fetch_user(&username).await?;
                let mut output = format!("{user}\n");
                if let Some(prefs) = lcdb::query_user_preferences(&user)? {
                    if let Some(announcement_prefs) = prefs.announcement {
                        output += "This user is currently being tracked.\n";
                        output += &format!("Failures are {}announced.\n",
                            if announcement_prefs.announce_failures { "" } else { "not " }
                        );
                        output += &format!("Submission links are {}abled.\n",
                            if announcement_prefs.has_submission_link { "en" } else { "dis" }
                        )
                    }
                }
                else {
                    output += "This user is not currently being tracked.";
                }

                output
            }
            "recent" => Self::get_recently_completed(parameters[0]).await?,
            "tracklist" => {
                let mut output = String::from("**Tracked users:**");
                let users = lcdb::query_tracked_users();
                match users {
                    Ok(users) => {
                        for user in users {
                            output += "\n\t";
                            output += &user.username;
                        }
                    }
                    Err(err) => {
                        output = format!("Error retrieving tracklist: {err}");
                    }
                }

                output
            }
            "track" => {
                let username = parameters
                    .first()
                    .context("Expected username for tracking, got none.")?
                    .to_string();

                let user = lcapi::fetch_user(&username).await?;
                lcdb::track_user(&user)
                    .inspect_err(|_| log::error!("Could not track user {username}"))?;

                react_ok().await?
            }
            "untrack" => {
                String::from("`untrack` is currently temporarily disabled.")
                // let username = parameters
                //     .first()
                //     .context("Expected username for untracking, got none.")?
                //     .to_string();

                // let user = lcapi::fetch_user(username).await?;
                // lcdb::untrack_user(&user)?;

                // msg.react(
                //     &ctx.http,
                //     serenity::all::ReactionType::Unicode(String::from("✅")),
                // )
                // .await?;
                // String::from("")
            }
            "prefs" => {
                let get_usage = || format!("Expected usage: `{}prefs <username> \
                                            [announce_fail|announce_link]=[true|false]`",
                                            getenv_call_token());

                let (username, pref_changes) = parameters
                    .split_first()
                    .with_context(get_usage)?;

                // Helps against a common pitfall with this command...
                if pref_changes.contains(&"=") {
                    return Err(anyhow!("{}\n (there mustn't be whitespace around the '`=`')", 
                               get_usage()))
                }

                // Get the User object
                let user = match lcdb::query_user(username)? {
                    Some(user) => user,
                    None => lcapi::fetch_user(username).await?
                };

                let mut prefs = lcdb::query_user_preferences(&user)?.unwrap_or_default();
                let mut msgs = Vec::new();

                for change in pref_changes {
                    let change_tuple = change
                        .split("=")
                        .next_tuple()
                        .map(|(cmd, state)| (cmd.trim(), state.trim()));

                    msgs.push(match change_tuple {
                        Some(("announce_fail", state @ ("true" | "false"))) => {
                            prefs.announcement = Some(prefs.announcement.map_or_else(
                                || AnnouncementPreferences {
                                    announce_failures: state == "true",
                                    has_submission_link: false },
                                |a| AnnouncementPreferences {
                                    announce_failures: state == "true",
                                    has_submission_link: a.has_submission_link }
                            ));

                            lcdb::update_user_preferences(&user, &prefs)?;
                            log::info!("Updated {username}'s announcement preferences: \
                                    announce_fail = {state}");

                            react_ok().await?
                        }
                        Some(("announce_link", state @ ("true" | "false"))) => {
                            prefs.announcement = Some(prefs.announcement.map_or_else(
                                || AnnouncementPreferences {
                                    announce_failures: false,
                                    has_submission_link: state == "true"},
                                |a| AnnouncementPreferences {
                                    announce_failures: a.announce_failures,
                                    has_submission_link: state == "true"}
                            ));

                            lcdb::update_user_preferences(&user, &prefs)?;
                            log::info!("Updated {username}'s announcement preferences: \
                                    announce_link = {state}");

                            react_ok().await?
                        }
                        Some((cmd @ ("announce_fail" | "announce_link"), state)) => {
                            return Err(anyhow!("Cannot set {cmd} to {state}: \n{}", get_usage()))
                        }
                        Some((unknown_cmd, _)) => {
                            return Err(anyhow!("Unknown announcement preference: {unknown_cmd} \n\
                                                {}", get_usage()))
                        }
                        None => {
                            return Err(anyhow!("Unknown announcement preference. \n{}", 
                                               get_usage()))
                        }
                    })
                }

                msgs.join("\n")
            }
            "help" => Self::get_help(),
            "clanker" => String::from("call me clanker one more mf time"),
            "insert" => {
                if !is_debug_mode() {
                    String::from("This command is only available in debug mode.")
                } else {
                    let (params, problem_name) = parameters.split_at_checked(2).context(
                        "Expected usage: `!insert <username> <success|failure> <problem_name>`",
                    )?;

                    let username = params
                        .first()
                        .context("Expected username for tracking, got none.")?
                        .to_string();

                    let user = lcapi::fetch_user(&username).await?;

                    let success = parameters
                        .get(1)
                        .context("Expected problem result (success | failure), got none.")?
                        .eq(&"success");

                    let problem = problem_name.join(" ");

                    log::info!("Inserted fake submission: {problem}");

                    lcdb::insert_fake_submission(&user, problem, success)?;

                    react_ok().await?
                }
            }
            _ => {
                if Commands::is_valid_cmd(command) {
                    log::info!("User submitted unknown command: {}", command);
                    return Err(anyhow!(
                        "No such command found: {}, see $help for commands.",
                        command
                    ));
                } else {
                    log::info!("User submitted invalid command: {}", command);
                    return Err(anyhow!("Invalid command syntax."));
                }
            }
        };

        Ok(result)
    }

    async fn get_recently_completed(username: &str) -> Result<String> {
        Ok(format!(
            "{}",
            lcapi::fetch_recently_completed(username)
                .await?
                .first()
                .context(format!("No recently completed problems for {}", username))?
        ))
    }
}

/// Non-async helpers
impl Commands {
    /// Ensures that the string slice conforms to C-like identifier regex
    fn is_valid_cmd(s: &str) -> bool {
        s.len() <= MAX_CMD_LENGTH
            && regex::Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$")
                .unwrap()
                .is_match(s)
    }

    /// Gets a help string. Should be updated after a new command is added
    /// TODO: Generate automatically?
    pub fn get_help() -> String {
        let t = getenv_call_token();
        format!(
            r#"
**Command List:**
`{t}audit <leetcode username>`:  Get stats on a leetcode user.
`{t}recent <leetcode username>`:  Get the most recent submission from a leetcode user.
`{t}track <leetcode username>`:  Track a user. This will cause the bot to begin tracking submissions for this user.
`{t}untrack <leetcode username>`:  Untrack a user.
`{t}prefs <leetcode username>`: Modify announcement preferences for a user.
`{t}tracklist`:  List all tracked users.
`{t}help`:  Get information on supported commands
"#,
        )
    }
}

/// Get the call token from the environment (.env file)
///
/// # Panics
/// If $BOT_CALL_TOKEN is not defined, or is more than a single character, will panic.
pub fn getenv_call_token() -> char {
    let env_token = std::env::var("BOT_CALL_TOKEN")
        .unwrap_or_else(|_| {
            log::error!("$BOT_CALL_TOKEN not defined. \n Please define a single-character \
                         call-token (i.e., $ or !)");
            panic!()
        });

    let token = env_token.chars().next().expect("BOT_CALL_TOKEN is empty.");
    if env_token.len() > 1 {
        log::warn!(
            "$BOT_CALL_TOKEN not a single character. Truncating to {}",
            token
        );
    }

    token
}

/// Returns whether we are in debug mode.
pub fn is_debug_mode() -> bool {
    getenv_call_token() == '!'
}