# Beat Leader Discord Bot

**Disclaimer**: I don't know what I'm doing, both in Rust and Discord territory. It may blow up in your face, be warned.

## What is this project?

A (not so) simple Discord bot providing the following commands:

- ``/bl-link`` / ``/bl-unlink``, allowing to link user account to Beat Leader profile. Not required if user has linked Discord account on BeatLeader website.  
- ``/bl-replay``, allowing to post replay according to set criteria along with links to BL replay and ArcViewer ![](docs/bl-replay.gif)
- ``/bl-profile``, allowing to post user profile ![](docs/bl-profile.gif)
- ``/bl-add-auto-role`` / ``/bl-remove-auto-role``, allowing a user (role management permission required) to configure the automatic setting of selected roles to server users based on their BL profile. The roles to be set up are grouped, and each role can be assigned a set of multiple conditions that must be met for it to be given. ![](docs/bl-role.gif)
- ``/bl-set-log-channel``, allowing to set the channel on which all role changes will be posted ![](docs/bl-log.gif)
- ``/bl-set-profile-verification``, allowing to set the profile verification requirement when linking a player's profile
- ``/bl-set-clan-invitation``, allowing to set up self-sending by the user invitations to the clan without the involvement of the clan owner (**NOTE**: requires contacting NSGolova on BeatLeader discord to get OAuth application id and secret)
- ``/bl-clan-invitation``, allowing a user to send an invitation to join a clan on their own
- ``/bl-clan-wars-playlist``, allowing a user to generate personalized playlist of clan wars maps
- ![](docs/clan-wars-playlist.png)
- ``/bl-set-clan-wars-maps-channel``, allowing to set the channel on which top 30 clan wars maps will be posted 
- ![](docs/clan-wars-maps.png)
- ``Capture the map`` context menu command, allowing you to check the pp and accuracy needed to capture the map after clicking on any message containing a link to the leaderboard
- ![](docs/capture-the-map.png)
- ``/bl-show-settings``, showing current server settings ![](docs/bl-show.gif)
- ``/bl-export`` / ``/bl-import``, allowing to export and import all bot data (bot owner only)

## Setup

All of the following commands require a Rust environment installed on your computer.

1. Copy ``config.example.toml`` as ``config.toml`` and/or ``config.dev.toml``. The first will be used production, the second for development. If both are present dev one takes precedence.
2. Register bot:
- Go to [Discord Developer Portal](https://discord.com/developers/applications)
- Create New Application
- Copy Discord Token (click ``Reset Token`` button on Bot tab to obtain it) and set ``discord_token`` in ``config.toml`` / ``config.dev.toml``
- If you want to use clan commands set ``client_id``, ``client_secret`` and ``redirect_url`` in the ``oauth`` section of ``config.toml`` (get them from NSGolova on BeatLeader discord). In addition, you need to run a server that will receive the OAuth Authorization code and generate the corresponding Discord command. You can use this [project](https://github.com/motzel/bl-bot-frontend) for this purpose or write your own.
3. Invite a bot to your server (**replace ``<APP_ID>`` with your application ID**, you can find it on General Information tab in Discord Developer Portal)
``https://discord.com/oauth2/authorize?client_id=<APP_ID>&scope=bot&permissions=2415937536``
   (required permissions: Manage roles, Embed links, Send Messages, Use Application Commands)
4. Either download the latest version for your server architecture from the [Releases](https://github.com/motzel/bl-bot/releases) page and run it, or build from source as described below. Make sure that the directory from which you run the bot includes the ``config.toml`` file, the ``assets`` and ``static`` directories from this repository, and empty ``.logs``, ``.storage`` and ``.http-cache`` directories.
5. The bot's built-in web server uses the HTTP protocol (defaults to port 3000, you can change this in ``config.toml``), so you probably need a SSL termination reverse proxy, such as nginx. Example nginx configuration (using Let's encrypt SSL certificate):
```
server {
   server_name your-domain.com;

   location / {
      proxy_set_header Host $http_host;
      proxy_set_header  X-Real-IP         $remote_addr;
      proxy_set_header  X-Forwarded-For   $proxy_add_x_forwarded_for;
      proxy_set_header  X-Forwarded-Proto $scheme;
      proxy_http_version 1.1;
      proxy_set_header Connection "";
      proxy_pass http://localhost:3000/;
   }

   access_log /var/log/nginx/your-domain.com-access.log;
   error_log /var/log/nginx/your-domain.com-error.log;

   listen 443 ssl;
   ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
   ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;
   include /etc/letsencrypt/options-ssl-nginx.conf;
   ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;
}

server {
   if ($host = your-domain.com) {
      return 301 https://$host$request_uri;
   }
   
   listen 80;

   server_name your-domain.com;
   return 404;
}
```


## Build and run
```bash
cargo test && cargo run
```

After you launch and invite the bot to your server, it will be visible in the list of users, but inaccessible. The bot does not automatically register Discord commands globally, you have to do it manually. To do this, after logging into the account that owns the bot, issue the command ``@BL Bot register`` (use the name you gave it). The bot will respond by displaying 4 buttons that allow you to register or delete commands globally or only on this server.

![](docs/register.png)

Note: If you register commands globally remember that [global commands can take up to 1 hour to update](https://discordnet.dev/guides/int_basics/application-commands/slash-commands/creating-slash-commands.html#:~:text=Note%3A%20Global%20commands%20will%20take,yet%20please%20follow%20this%20guide.). During development, it is better to register them only on the test server, because they update immediately.




## Setting up an automatic deployment using Github Actions

The description applies to Ubuntu Server 22.04 (x86_64), but it will look similar for other Linux distributions.

1. Add ``deploy`` user and create ``deployments`` directory

```bash
sudo useradd -m -s /sbin/bash -d /home/deploy deploy
sudo -u deploy bash -c "mkdir -p /home/deploy/deployments"
```

2. Install ``supervisor``

```bash
sudo apt install supervisor
```

3. Update ``/etc/supervisor/supervisord.conf``

```tom
[unix_http_server]
file=/tmp/supervisor.sock
chown=deploy:deploy
chmod=0700

... 

[supervisorctl]
serverurl=unix:///tmp/supervisor.sock
```

4. Restart ``supervisor`` service

```bash
 systemctl restart supervisor
```

5. Add process configuration ``/etc/supervisor/conf.d/your-program-name.conf``

```
[program:your-program-name]
directory=/home/deploy/deployments/your-program-name
command=/home/deploy/deployments/your-program-name/your-program-name
environment=RUST_LOG="your_program_name=debug"
autostart=true
autorestart=true
stopasgroup=true
killasgroup=true
user=deploy
numprocs=1
redirect_stderr=true
stdout_logfile=/home/deploy/deployments/your-program-name/.logs/your-program-name.log
stdout_logfile_maxbytes=20MB
stdout_logfile_backups=30
stopwaitsecs=30
```

6. Update config and start program

```bash
supervisorctl update your-program-name
```

7. Update program name in ``.deploy/deploy.sh`` and possibly the architecture of your server in ``.github/workflows/release.yml``


8. Create SSH keys

```bash
ssh-keygen -t ed25519 -C "your_email@example.com"
```
and copy public key contents to ``/home/deploy/.ssh/authorized_keys`` on your server. Make sure that both ``.ssh`` directory and ``/home/deploy/.ssh/authorized_keys`` file are owned by ``deploy:deploy`` and have ``0700`` and ``0600`` permissions, respectively.

9. Add GitHub actions secrets (Settings/Secrets and variables/Actions)
- ``SSH_PRIVATE_KEY`` - copy contents of your private SSH key
- ``SSH_HOST`` - set it to your server IP address
- ``SSH_USER`` - set it to ``deploy``
- ``DEPLOY_PATH`` - set it to ``/home/deploy/deployments``

10. Create ``/home/deployments/YOUR-PROGRAM-NAME/.http-cache``, ``/home/deployments/YOUR-PROGRAM-NAME/.logs`` and ``/home/deploy/deployments/YOUR-PROGRAM-NAME/.storage`` directories. Copy ``assets`` folder as ``/home/deploy/deployments/YOUR-PROGRAM-NAME/assets``. Copy ``static`` folder as ``/home/deploy/deployments/YOUR-PROGRAM-NAME/static``. Copy ``config.example.toml`` as ``/home/deploy/deployments/YOUR-PROGRAM-NAME/config.toml`` and set it up.
 
Every time you want to deploy a new version, just add the ``vX.Y.Z`` (e.g. ``v0.1.1``) tag to your commit and push the code to the repository.
