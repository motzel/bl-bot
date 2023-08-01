use crate::{Context, Error};
use poise::serenity_prelude as serenity;

#[poise::command(prefix_command, rename = "register")]
pub(crate) async fn cmd_register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
