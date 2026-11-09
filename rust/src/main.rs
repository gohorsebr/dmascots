// No console window on Windows; the CLI subcommands attach to the parent console instead.
#![cfg_attr(windows, windows_subsystem = "windows")]

//! Entry point: starts the app (or hands off to the running one), or runs one of the
//! commands for a running app: list, add, remove, say, steal, quit (port of app.py + cli.py).

use std::io::Read;
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use serde_json::{json, Value};

use desktop_mascots::app::{self, clean_text, Herd, Options};
use desktop_mascots::{characters, ipc, platform};

const TARGET_HELP: &str = "a mascot id (see 'list') or name";

fn names() -> String {
    characters::names().join(", ")
}

#[derive(Parser)]
#[command(
    name = "desktop-mascots",
    version,
    about = "Mascots that live on top of your windows.",
    after_help = "Commands for the running app: list, add, remove, say, steal, quit \
                  (see 'desktop-mascots COMMAND --help')."
)]
struct Cli {
    /// Mascots to spawn, repeat for more; default: one of each
    #[arg(value_name = "MASCOT")]
    mascots: Vec<String>,
    /// Size multiplier
    #[arg(long, default_value_t = 1.0)]
    scale: f64,
    /// The mascots leave after this many seconds (and the app exits if none are left)
    #[arg(long, value_name = "SECONDS")]
    time: Option<f64>,
    /// The mascots start by saying TEXT ('-' reads it from stdin)
    #[arg(long, value_name = "TEXT")]
    say: Option<String>,
    /// Clicks pass straight through the mascots (no poking or dragging)
    #[arg(long)]
    click_through: bool,
    /// Right-clicking a mascot opens its menu (off by default)
    #[arg(long)]
    menu: bool,
    /// No tray icon; quit with Ctrl+C or 'desktop-mascots quit'
    #[arg(long)]
    no_tray: bool,
    /// Linux/X11: let the window manager manage the windows instead of bypassing it
    #[arg(long)]
    managed: bool,
}

#[derive(Parser)]
#[command(name = "desktop-mascots list", about = "Show the mascots that are on screen.")]
struct ListCli {
    /// Machine-readable output
    #[arg(long)]
    json: bool,
}

#[derive(Parser)]
#[command(name = "desktop-mascots add", about = "Add mascots to the running app.")]
struct AddCli {
    /// What to add, repeat for more
    #[arg(required = true, value_name = "MASCOT")]
    mascots: Vec<String>,
    /// They leave after this many seconds
    #[arg(long, value_name = "SECONDS")]
    time: Option<f64>,
    /// Their opening line ('-' reads it from stdin)
    #[arg(long, value_name = "TEXT")]
    say: Option<String>,
}

#[derive(Parser)]
#[command(
    name = "desktop-mascots remove",
    about = "Remove mascots: by id (see 'list'), every one of a kind by name, or --all."
)]
struct RemoveCli {
    /// A mascot id (see 'list') or name
    #[arg(value_name = "TARGET")]
    targets: Vec<String>,
    /// Remove every mascot (the app keeps running)
    #[arg(long)]
    all: bool,
}

#[derive(Parser)]
#[command(name = "desktop-mascots say", about = "Make the running mascots say something.")]
struct SayCli {
    /// What to say ('-' reads it from stdin)
    #[arg(required = true)]
    text: Vec<String>,
    /// Who says it: all (default), a mascot id (see 'list') or name
    #[arg(long, default_value = "all", value_name = "TARGET")]
    to: String,
    /// How long the bubble stays (default: based on length)
    #[arg(long)]
    seconds: Option<f64>,
}

#[derive(Parser)]
#[command(
    name = "desktop-mascots steal",
    about = "A mascot runs to your pointer, grabs it and runs off with it. Click the thief to get it back early."
)]
struct StealCli {
    /// Who steals it: any (default, picked at random), a mascot id (see 'list') or name
    #[arg(long, default_value = "any", value_name = "TARGET")]
    to: String,
    /// How long it holds on to the pointer (max 60)
    #[arg(long, default_value_t = 5.0)]
    seconds: f64,
}

#[derive(Parser)]
#[command(name = "desktop-mascots quit", about = "Close the running mascots.")]
struct QuitCli {}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("{msg}");
    ExitCode::FAILURE
}

fn usage_error<T: CommandFactory>(msg: impl std::fmt::Display) -> ! {
    T::command().error(clap::error::ErrorKind::ValueValidation, msg).exit()
}

fn read_stdin() -> String {
    let mut s = String::new();
    let _ = std::io::stdin().read_to_string(&mut s);
    s
}

/// '-' reads stdin; the text becomes one paragraph (the bubble wraps it).
fn read_text(words: &[String]) -> String {
    let text = if words == ["-"] { read_stdin() } else { words.join(" ") };
    clean_text(&text)
}

fn check_target<T: CommandFactory>(target: &str, everyone: &str) {
    let is_id = !target.is_empty() && target.bytes().all(|b| b.is_ascii_digit());
    if target != everyone && !is_id && characters::get(target).is_none() {
        usage_error::<T>(format!("unknown mascot: {target} (use {TARGET_HELP}, or {everyone})"));
    }
}

fn parse<T: Parser>(name: &str, args: &[String]) -> T {
    T::parse_from(std::iter::once(name.to_string()).chain(args.iter().cloned()))
}

/// Send one request; Err(exit code) when the app isn't running or refuses.
fn request(msg: Value) -> Result<Value, ExitCode> {
    let Some(reply) = ipc::send(&msg) else {
        return Err(fail("desktop-mascots isn't running (start it with: desktop-mascots)"));
    };
    if reply.get("ok") != Some(&Value::Bool(true)) {
        let err = reply.get("error").and_then(Value::as_str).unwrap_or("unknown error");
        return Err(fail(format!("error: {err}")));
    }
    Ok(reply)
}

fn mascots(reply: &Value) -> Vec<Value> {
    reply.get("mascots").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn label(m: &Value) -> String {
    format!("{} #{}", m["name"].as_str().unwrap_or("?"), m["id"])
}

fn labels(ms: &[Value]) -> String {
    ms.iter().map(label).collect::<Vec<_>>().join(", ")
}

// ----------------------------------------------------------------- commands

fn list_command(args: &[String]) -> ExitCode {
    let cli: ListCli = parse("list", args);
    let reply = match request(json!({"cmd": "list"})) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let ms = mascots(&reply);
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&ms).unwrap_or_default());
        return ExitCode::SUCCESS;
    }
    if ms.is_empty() {
        println!("No mascots on screen. Add some with: desktop-mascots add MASCOT");
        return ExitCode::SUCCESS;
    }
    println!("{:>3}  {:<9} {:<9} {:<13} NOTES", "ID", "MASCOT", "DOING", "WHERE");
    for m in &ms {
        let mut notes = Vec::new();
        if let Some(left) = m["leaves_in"].as_f64() {
            notes.push(format!("leaves in {left:.0}s"));
        }
        if m["holding_pointer"].as_bool() == Some(true) {
            notes.push("holding your pointer".to_string());
        }
        let s = |k: &str| m[k].as_str().unwrap_or("").to_string();
        let n = |k: &str| m[k].as_i64().unwrap_or(0);
        println!(
            "{:>3}  {:<9} {:<9} {:>5},{:<7} {}",
            n("id"),
            s("name"),
            s("state"),
            n("x"),
            n("y"),
            notes.join(", ")
        );
    }
    ExitCode::SUCCESS
}

fn add_command(args: &[String]) -> ExitCode {
    let cli: AddCli = parse("add", args);
    let unknown: Vec<&str> = cli.mascots.iter().filter(|m| characters::get(m).is_none()).map(String::as_str).collect();
    if !unknown.is_empty() {
        usage_error::<AddCli>(format!("unknown mascot(s): {} (available: {})", unknown.join(", "), names()));
    }
    if cli.time.is_some_and(|t| t <= 0.0) {
        usage_error::<AddCli>("--time must be greater than 0");
    }
    let say = cli.say.as_ref().map(|s| read_text(std::slice::from_ref(s)));
    match request(json!({"cmd": "spawn", "mascots": cli.mascots, "lifetime": cli.time, "say": say})) {
        Err(code) => code,
        Ok(reply) => {
            let added = labels(&mascots(&reply));
            println!("added {}", if added.is_empty() { cli.mascots.join(", ") } else { added });
            ExitCode::SUCCESS
        }
    }
}

fn remove_command(args: &[String]) -> ExitCode {
    let cli: RemoveCli = parse("remove", args);
    if cli.targets.is_empty() && !cli.all {
        usage_error::<RemoveCli>("say which mascots to remove (an id or name), or --all");
    }
    for t in &cli.targets {
        check_target::<RemoveCli>(t, "all");
    }
    let targets = if cli.all { vec!["all".to_string()] } else { cli.targets.clone() };
    match request(json!({"cmd": "remove", "targets": targets})) {
        Err(code) => code,
        Ok(reply) => {
            let gone = mascots(&reply);
            if gone.is_empty() {
                return fail(format!("no mascot matched: {}", cli.targets.join(" ")));
            }
            println!("removed {}", labels(&gone));
            ExitCode::SUCCESS
        }
    }
}

fn say_command(args: &[String]) -> ExitCode {
    let cli: SayCli = parse("say", args);
    check_target::<SayCli>(&cli.to, "all");
    let text = read_text(&cli.text);
    if text.is_empty() {
        usage_error::<SayCli>("nothing to say");
    }
    match request(json!({"cmd": "say", "text": text, "to": cli.to, "seconds": cli.seconds})) {
        Err(code) => code,
        Ok(reply) if reply.get("count").and_then(Value::as_u64).unwrap_or(0) == 0 => {
            fail(format!("no {} on screen right now", cli.to))
        }
        Ok(_) => ExitCode::SUCCESS,
    }
}

fn steal_command(args: &[String]) -> ExitCode {
    let cli: StealCli = parse("steal", args);
    check_target::<StealCli>(&cli.to, "any");
    if !(cli.seconds > 0.0 && cli.seconds <= 60.0) {
        usage_error::<StealCli>("--seconds must be more than 0 and at most 60");
    }
    match request(json!({"cmd": "steal", "to": cli.to, "seconds": cli.seconds})) {
        Err(code) => code,
        Ok(_) => ExitCode::SUCCESS,
    }
}

fn quit_command(args: &[String]) -> ExitCode {
    let _: QuitCli = parse("quit", args);
    match request(json!({"cmd": "quit"})) {
        Err(code) => code,
        Ok(_) => ExitCode::SUCCESS,
    }
}

// ----------------------------------------------------------------- startup

fn install_signal_handlers() {
    #[cfg(unix)]
    {
        extern "C" fn on_signal(_: libc::c_int) {
            app::QUIT.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // SAFETY: the handler only stores to an atomic
        unsafe {
            libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t); // Ctrl+C in the terminal
            libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t); // `kill`: still exit cleanly
        }
    }
    #[cfg(windows)]
    {
        unsafe extern "system" fn on_ctrl(_: u32) -> i32 {
            app::QUIT.store(true, std::sync::atomic::Ordering::Relaxed);
            1
        }
        // SAFETY: registering a handler that only stores to an atomic
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(on_ctrl), 1);
        }
    }
}

/// Windows GUI-subsystem builds have no console: borrow the terminal we were started from,
/// so commands, `--help` and errors print where you typed them.
fn attach_console() {
    #[cfg(windows)]
    // SAFETY: plain Win32 call; failure just means there's no parent console
    unsafe {
        use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() -> ExitCode {
    attach_console();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let rest = argv.get(1..).unwrap_or(&[]);
    match argv.first().map(String::as_str) {
        Some("list") => return list_command(rest),
        Some("add") => return add_command(rest),
        Some("remove") => return remove_command(rest),
        Some("say") => return say_command(rest),
        Some("steal") => return steal_command(rest),
        Some("quit") => return quit_command(rest),
        _ => {}
    }

    let mut cli = Cli::parse();
    let unknown: Vec<&str> = cli.mascots.iter().filter(|m| characters::get(m).is_none()).map(String::as_str).collect();
    if !unknown.is_empty() {
        usage_error::<Cli>(format!("unknown mascot(s): {} (available: {})", unknown.join(", "), names()));
    }
    if cli.time.is_some_and(|t| t <= 0.0) {
        usage_error::<Cli>("--time must be greater than 0");
    }
    if cli.say.as_deref() == Some("-") {
        cli.say = Some(read_stdin());
    }
    let wanted: Vec<String> = if cli.mascots.is_empty() {
        characters::names().into_iter().map(String::from).collect()
    } else {
        cli.mascots.clone()
    };

    // Already running? Hand the new mascots to that instance instead of starting a second one.
    if let Some(reply) = ipc::send(&json!({"cmd": "spawn", "mascots": wanted, "lifetime": cli.time, "say": cli.say})) {
        let added = labels(&mascots(&reply));
        let added = if added.is_empty() { wanted.join(", ") } else { added };
        let leaving = cli.time.map(|t| format!(" (leaving in {t}s)")).unwrap_or_default();
        println!("desktop-mascots is already running; added {added}{leaving}.");
        return ExitCode::SUCCESS;
    }

    let backend = match platform::open(cli.managed) {
        Ok(b) => b,
        Err(e) => return fail(format!("error: {e}")),
    };
    install_signal_handlers();
    let requests = match ipc::serve() {
        Ok(rx) => Some(rx),
        Err(e) => {
            eprintln!("warning: couldn't open the control socket ({e}); commands won't reach this instance");
            None
        }
    };
    let mut herd = Herd::new(
        backend,
        &Options {
            scale: cli.scale,
            tray: !cli.no_tray,
            menu: cli.menu,
            click_through: cli.click_through,
            quit_when_empty: cli.time.is_some(),
        },
    );
    for name in &wanted {
        herd.spawn(characters::get(name).unwrap(), cli.time, cli.say.as_deref());
    }
    let code = herd.run(requests.as_ref());
    if requests.is_some() {
        ipc::cleanup();
    }
    ExitCode::from(code as u8)
}
