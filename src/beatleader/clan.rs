use crate::beatleader::Client;

pub struct ClanResource<'a> {
    client: &'a Client,
}

impl<'a> ClanResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }
}

pub(crate) type ClanTag = String;
