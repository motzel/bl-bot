pub(crate) mod auto_role_add;
pub(crate) mod auto_role_remove;
pub(crate) mod auto_role_show;
pub(crate) mod link;
pub(crate) mod replay;
pub(crate) mod unlink;

pub(crate) use auto_role_add::cmd_add_auto_role;
pub(crate) use auto_role_remove::cmd_remove_auto_role;
pub(crate) use auto_role_show::cmd_show_auto_roles;
pub(crate) use link::cmd_link;
pub(crate) use replay::cmd_replay;
pub(crate) use unlink::cmd_unlink;
