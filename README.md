## Basic Usage

#### Requirements
- Git
- Cargo (Rustup)

##### Step 1 - Build the project
Simply do `cargo build --release`. It might take a while.

##### Step 2 - Write the Script that Igneous should follow

The script format is simple. There are 3 command types:
- `T!bot? are you there?` - wait for anyone (including other bots) to type the preset string "bot? are you there?"
- `R!yeah, I'm here` - reply with the preset string "yeah, I'm here"
- `IMG![images/bot-selfie.png]` - reply with image

All commands **must** have a timeout (the timeout **can** be `0.0`) after the command executes:
`T!bot? are you there?+3.0` - wait 3 seconds after the user types the preset string

If the first command is a trigger, then the bot will only become active when the requirement for input is met. Otherwise, any message will activate the bot.

A simple ping-pong bot:
```
T!bot? are you there?+2.0
R!yeah, I'm here+0.0
T!ok+2.0
R!soooooo, why did you call me?+0.0
IMG![Confused.png]+0.0
```

Save the file.

##### Step 3 - Run

Use the following command to run the bot:

`DISCORD_TOKEN="<your discord bot token>" ./target/release/Igneous <path to your script file>`