mod app;
mod clipboard;
mod config;
mod editor;
mod event;
mod favorites;
mod history;
mod otp_setup;
mod pass;
mod picker;
mod recovery;
mod search;
mod text_input;
mod totp;
mod ui;

use app::App;
use event::AppEvent;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--clipboard-helper" => {
                return clipboard::run_helper(
                    args.get(2)
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(45000),
                );
            }
            "--install-otp" => {
                if otp_setup::available() {
                    println!("OTP support is already installed.");
                    return Ok(());
                }
                let plan = otp_setup::detect().map_err(anyhow::Error::msg)?;
                return otp_setup::install(&plan).map_err(anyhow::Error::msg);
            }
            "--pick" => {
                install_panic_hook();
                let mut terminal = ratatui::init();
                let _paste = event::PasteGuard::enable()?;
                let result = picker::run(&mut terminal);
                ratatui::restore();
                return result;
            }
            "-h" | "--help" => {
                println!("PassTUI — A lazygit-style terminal UI for `pass`\n");
                println!("USAGE:");
                println!("    passtui [OPTIONS]\n");
                println!("OPTIONS:");
                println!("    -h, --help       Print help information");
                println!("    -V, --version    Print version information");
                println!("    --pick           Pick a password and copy it to the clipboard");
                println!(
                    "    --install-otp    Install OTP support with the native package manager"
                );
                return Ok(());
            }
            "-V" | "--version" => {
                println!("passtui {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }

    // Install a panic hook that restores the terminal before printing the panic.
    install_panic_hook();

    let mut terminal = ratatui::init();
    let _paste = event::PasteGuard::enable()?;
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        event::disable_paste();
        ratatui::restore();
        original_hook(info);
    }));
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
    let mut app = App::new();

    loop {
        if let Some(plan) = app.install_request.take() {
            event::disable_paste();
            ratatui::restore();
            let result = otp_setup::install(&plan);
            *terminal = ratatui::init();
            crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
            app.finish_otp_install(result);
        }
        app.tick();
        // Draw
        terminal.draw(|frame| ui::layout::render(frame, &mut app))?;

        if !app.running {
            break;
        }

        // Poll events (100 ms tick for auto-dismiss timers)
        match event::poll_event(Duration::from_millis(100))? {
            Some(AppEvent::Key(key)) => app.handle_key(key),
            Some(AppEvent::Paste(text)) => app.handle_paste(&zeroize::Zeroizing::new(text)),
            Some(AppEvent::Tick) => app.tick(),
            None => {}
        }
    }

    Ok(())
}
