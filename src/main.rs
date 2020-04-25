use std::env;

use serenity::{
    model::{channel::Message, gateway::Ready},
    prelude::*,
};

mod lib;
use lib::*;

fn main() {
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");
    let handler = Handler::new();
    let mut client = Client::new(&token, handler).expect("Error creating client");
    if let Err(why) = client.start() {
        eprintln!("Client error: {:?}", why);
    }
}
