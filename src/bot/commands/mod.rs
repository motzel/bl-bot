pub(crate) use backup::{cmd_export, cmd_import};
pub(crate) use clan::{
    cmd_clan_invitation, cmd_invite_player, cmd_set_clan_invitation, cmd_set_clan_invitation_code,
};
pub(crate) use guild::{
    cmd_add_auto_role, cmd_remove_auto_role, cmd_set_log_channel, cmd_set_profile_verification,
    cmd_show_settings,
};
pub(crate) use player::{cmd_link, cmd_profile, cmd_refresh_scores, cmd_replay, cmd_unlink};
pub(crate) use register::cmd_register;

pub(crate) mod backup;
pub(crate) mod clan;
pub(crate) mod guild;
pub(crate) mod player;
pub(crate) mod register;
