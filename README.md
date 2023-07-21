# Beat Leader Discord Bot

**Disclaimer**: I don't know what I'm doing, both in Rust and Discord territory. It may blow up in your face, be warned.

## Setup

1. Copy ``Secrets.example.toml`` as ``Secrets.toml``
2. Register bot:
- Go to (Discord Developer Portal)[https://discord.com/developers/applications]
- Create New Application
- Copy Discord Token (Reset token button on Bot tab) and set ``DISCORD_TOKEN`` in ``Secrets.toml``
- Right click on your server name, copy server ID and set ``GUILD_ID`` in ``Secrets.toml``
3. Invite a bot to your server (*replace ``<APP_ID>` with your application ID, you can find it on General Information tab in Discord Developer Portal)
https://discord.com/oauth2/authorize?client_id=<APP_ID>&scope=bot&permissions=2415937536
4. Create [shuttle.rs](https://www.shuttle.rs/) account
5. ``cargo install cargo-shuttle``
6. ``cargo shuttle login``

## Deploy

```bash
cargo shuttle deploy
```

## Develop
```bash
cargo shuttle run
```