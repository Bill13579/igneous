use std::thread;
use std::env;
use std::collections::HashMap;
use std::process;
use std::fs;
use std::sync::{Mutex, RwLock};
use std::sync::mpsc;
use std::sync::Arc;
use std::time;
use std::path::Path;
use rusqlite::{params, Connection, Result};
use rusqlite::NO_PARAMS;

use strsim::{levenshtein};

use regex::Regex;

use serenity::{
    model::{channel::Message, gateway::Ready, id::ChannelId, user::CurrentUser},
    prelude::*,
    utils::MessageBuilder,
    http::AttachmentType,
};

pub fn strip_stylization(s: &str) -> String {
    let re = Regex::new(r"[!?. …',’-]*").unwrap();
    let sr = s.to_lowercase();
    let sr = re.replace_all(&sr, "").into_owned().to_string();
    sr
}

pub struct Handler {
    states: Arc<Vec<State>>,
    statusMap: Arc<Mutex<HashMap<u64, Option<std::sync::mpsc::Sender<Message>>>>>,
    db: Arc<Mutex<Connection>>,
}
impl Handler {
    pub fn new() -> Handler {
        let conn = Connection::open("attrs.db").unwrap();
        Handler {
            states: Arc::new(Handler::gen_state()),
            statusMap: Arc::new(Mutex::new(HashMap::new())),
            db: Arc::new(Mutex::new(conn)),
        }
    }
    fn gen_state() -> Vec<State> {
        let mut args = env::args();
        args.next();
        let flow_file = args.next().unwrap_or_else(|| {
            eprintln!("flow file not specified");
            process::exit(1);
        });
        println!("Reading from {}", flow_file);
        let contents = fs::read_to_string(flow_file)
            .unwrap_or_else(|e| {
                eprintln!("error reading flow file: {:?}", e);
                process::exit(1);
            });
        let mut states = Vec::new();
        let commands = contents.split("\n").map(|v| v.trim().to_owned()).filter(|v| v != "");
        let command_matcher = Regex::new(r"^([a-zA-Z_]*?)!(?:\[?)(.*?)(?:\]?)\+([0-9.]*)$").unwrap();
        for c in commands {
            let matched = command_matcher.captures_iter(&c).next().unwrap();
            let cmd_type: String = matched.get(1).unwrap().as_str().to_owned();
            let data: String = matched.get(2).unwrap().as_str().to_owned();
            let timeout: String = matched.get(3).unwrap().as_str().to_owned();
            let timeout: f32 = timeout.parse().unwrap();
            states.push(match cmd_type.as_str() {
                "R" => State::Display(data, timeout),
                "IMG" => State::DisplayImage(data, timeout),
                "T" => State::Trigger(Requirement::new(data), timeout),
                "GO_OFFLINE" => State::GoOffline(timeout),
                "GO_ONLINE" => State::GoOnline(timeout),
                &_ => {
                    eprintln!("Invalid command: {}", cmd_type);
                    process::exit(1);
                }
            });
        }
        states.push(State::End);
        states
    }
}
impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        let entry = &mut self.statusMap.lock().unwrap();
        let entry = entry.entry(msg.channel_id.0).or_insert(None);
        let mut timeout = 0.0;
        if let None = entry {
            let (continue_or_not, t) = (&self.states[0]).check(&msg.content);
            if continue_or_not {
                timeout = t;

                let mut i = 0;
                let states = self.states.clone();
                let db = self.db.clone();
                let statusMap = self.statusMap.clone();
                let mut channel_id = msg.channel_id;

                let guard = db.lock().unwrap();
                let original_user = format!("u{}", msg.author.id.0.to_string());
                guard.execute(
                    &format!(
                        "create table if not exists {} (
                            id    TEXT PRIMARY KEY,
                            val   TEXT NOT NULL
                        )"
                    , original_user),
                    NO_PARAMS,
                ).unwrap();
                std::mem::drop(guard);

                let (tx, rx) = mpsc::channel::<Message>();
                tx.send(msg);
                *entry = Some(tx);

                let var_def = regex::Regex::new(";;(.*?);;").unwrap();
                let var_val = regex::Regex::new("<(.*?)>").unwrap();

                thread::spawn(move || {
                    loop {
                        match &states[i] {
                            State::Trigger(req, t) => {
                                let mut msg = rx.recv().unwrap_or_else(|e| {
                                    process::exit(1);
                                });
                                let mut required = vec![];
                                for m in var_def.captures_iter(&req.original()) {
                                    required.push(m.get(1).unwrap().as_str().to_owned());
                                }
                                if required.len() > 0 {
                                    let mut success = false;
                                    while !success {
                                        while msg.author.id.0 == ctx.cache.read().user.id.0 {
                                            msg = rx.recv().unwrap_or_else(|e| {
                                                process::exit(1);
                                            });
                                        }

                                        let mut new_pat = Regex::new(&var_def.replace_all(&regex::escape(&req.original()), "(.*?)").into_owned().to_string());
                                        let new_pat = match new_pat {
                                            Ok(p) => p,
                                            Err(e) => {
                                                return;
                                            }
                                        };

                                        let guard = db.lock().unwrap();
                                        let mut number_of_matches = 0;
                                        for (c, n) in new_pat.captures_iter(&msg.content).zip(required.iter()) {
                                            number_of_matches += 1;
                                            let c = c.get(1).unwrap().as_str().to_owned();
                                            guard.execute(
                                                &format!(
                                                    "INSERT OR IGNORE INTO {} VALUES (?, ?)"
                                                , original_user),
                                                params![n, &c],
                                            ).unwrap();
                                            guard.execute(
                                                &format!(
                                                    "UPDATE {} SET val=? WHERE id=?"
                                                , original_user),
                                                params![&c, n],
                                            ).unwrap();
                                        }
                                        std::mem::drop(guard);

                                        if required.len() == number_of_matches {
                                            success = true;
                                        }
                                    }
                                } else {
                                    while (msg.author.id.0 == ctx.cache.read().user.id.0) || !req.check(&msg.content) {
                                        msg = rx.recv().unwrap_or_else(|e| {
                                            process::exit(1);
                                        });
                                    }
                                }
                                timeout = *t;
                            },
                            State::Display(s, t) => {
                                let guard = db.lock().unwrap();
                                let s = var_val.replace_all(&s, |caps: &regex::Captures| {
                                    let mut stmt = guard.prepare(
                                        &format!(
                                            "SELECT val FROM {} WHERE id=?"
                                        , original_user)
                                    ).unwrap();
                                    let mut replace_with = String::from("");
                                    let attr_iter = stmt.query_map(params![caps.get(1).unwrap().as_str()], |row| {
                                        row.get(0)
                                    }).unwrap();
                                    for attr in attr_iter {
                                        replace_with = attr.unwrap();
                                    }
                                    replace_with
                                });
                                std::mem::drop(guard);
                                if let Err(why) = channel_id.say(&ctx.http, &s) {
                                    eprintln!("Error sending reply: {:?}", why);
                                }
                                timeout = *t;
                            },
                            State::DisplayImage(u, t) => {
                                let res = channel_id.send_message(&ctx.http, |m| {
                                    m.add_file(AttachmentType::Path(Path::new(u)));
                                    m
                                });
                                if let Err(why) = res {
                                    eprintln!("Error sending reply: {:?}", why);
                                }
                                timeout = *t;
                            },
                            State::GoOffline(t) => {
                                ctx.invisible();
                                timeout = *t;
                            },
                            State::GoOnline(t) => {
                                ctx.online();
                                timeout = *t;
                            },
                            State::End => {
                                let entry = &mut statusMap.lock().unwrap();
                                let entry = entry.entry(channel_id.0).or_insert(None);
                                *entry = None;
                                break;
                            }
                        }
                        i += 1;
                        thread::sleep(time::Duration::from_millis((1000.0 * timeout).round() as u64));
                    }
                });
            }
        } else if let Some(tx) = entry {
            tx.send(msg);
        }
    }
    fn ready(&self, _: Context, ready: Ready) {
        println!("What is this Discord thingy?");
    }
}

pub enum State {
    Trigger(Requirement, f32),
    Display(String, f32),
    DisplayImage(String, f32),
    GoOffline(f32),
    GoOnline(f32),
    End,
}

impl State {
    fn check(&self, input: &str) -> (bool, f32) {
        match self {
            State::Trigger(req, t) => (req.check(input), *t),
            State::Display(_s, t) => (true, *t),
            State::DisplayImage(_s, t) => (true, *t),
            State::GoOffline(t) => (true, *t),
            State::GoOnline(t) => (true, *t),
            State::End => (true, 0.0)
        }
    }
}

pub struct Requirement {
    original: String,
    original_stripped: String
}

impl Requirement {
    fn new(original: String) -> Requirement {
        Requirement {
            original_stripped: strip_stylization(&original),
            original,
        }
    }
    fn original(&self) -> String {
        self.original.clone()
    }
    fn check(&self, input: &str) -> bool {
        let similarity = levenshtein(&strip_stylization(input), &self.original_stripped);
        let diff = similarity as f32 / self.original_stripped.len() as f32;
        diff < 0.3
    }
}