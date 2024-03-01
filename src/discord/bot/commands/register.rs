use crate::discord::Context;
use crate::Error;

#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:register")]
#[poise::command(prefix_command, rename = "register", hide_in_help)]
pub(crate) async fn cmd_register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
