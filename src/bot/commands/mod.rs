pub(crate) use player::{cmd_link, cmd_replay, cmd_unlink};
pub(crate) use register::cmd_register;
pub(crate) use role::{cmd_add_auto_role, cmd_remove_auto_role};
pub(crate) use settings::{cmd_set_log_channel, cmd_show_settings};

pub(crate) mod player;
pub(crate) mod register;
pub(crate) mod role;
pub(crate) mod settings;
