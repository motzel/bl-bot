pub(crate) mod auto_role_add;
pub(crate) mod auto_role_remove;
pub(crate) mod auto_role_show;
pub(crate) mod link;
pub(crate) mod register;
pub(crate) mod replay;
pub(crate) mod unlink;

use crate::bot::GuildSettings;
use crate::{Context, Error};
pub(crate) use auto_role_add::cmd_add_auto_role;
pub(crate) use auto_role_remove::cmd_remove_auto_role;
pub(crate) use auto_role_show::cmd_show_auto_roles;
use futures::{Stream, StreamExt};
pub(crate) use link::cmd_link;
use poise::serenity_prelude as serenity;
pub(crate) use register::cmd_register;
pub(crate) use replay::cmd_replay;
pub(crate) use unlink::cmd_unlink;

async fn autocomplete_role_group<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    if let Some(_guild_id) = ctx.guild_id() {
        let group_names: Vec<String> = ctx
            .data()
            .guild_settings
            .lock()
            .await
            .get_groups()
            .iter()
            .filter(|rs| rs.contains(partial))
            .map(|s| s.to_string())
            .collect();

        futures::stream::iter(group_names)
    } else {
        futures::stream::iter(Vec::<String>::new())
    }
}
