## Basic Usage

#### Requirements
- Git
- Cargo (Rustup)

##### Step 1 - Build the project
Simply do `cargo build --release`. It might take a while.

##### Step 2 - Write the Script that Igneous should follow

The script format is simple. There are 3 command types:
`S!bot? are you there?` - wait for user to type the preset string ("bot? are you there?")
`I!yeah, I'm here` - reply with the preset string ("yeah, I'm here")
`IMG![images/bot-selfie.png]` - reply with image

All commands **must** have a timeout (the timeout **can** be `0.0`) after the command executes:
`S!bot? are you there?+3.0` - wait 3 seconds after the user types the preset string

Always start a bot with an `S!`. That would be your trigger.

A simple ping-pong bot:
```
S!bot? are you there?+2.0
I!yeah, I'm here+0.0
S!ok+2.0
I!soooooo, why did you call me?+0.0
IMG![Confused.png]+0.0
```

Save the file.

##### Step 3 - Run

Use the following command to run the bot:

`DISCORD_TOKEN="<your discord bot token>" ./target/release/Igneous <path to your script file>`