pub(crate) use guild::{
    cmd_add_auto_role, cmd_remove_auto_role, cmd_set_log_channel, cmd_show_settings,
};
pub(crate) use player::{cmd_link, cmd_profile, cmd_replay, cmd_unlink};
pub(crate) use register::cmd_register;

pub(crate) mod guild;
pub(crate) mod player;
pub(crate) mod register;
