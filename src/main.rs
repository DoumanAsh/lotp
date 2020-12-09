use std::{path, io};
use std::io::{BufRead, Write};
use std::fs::File;

use std::collections::BTreeMap;

const HELP: &'static str = "1. add <label> <data>
2. show <label>
3. remove <label>
4. help
5. exit
";
const VERSION_KEY: &'static str = "__version__";
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

const CONFIG: &'static str = ".lotp.json";

fn read_config(config: &path::Path) -> BTreeMap<u128, Vec<u8>> {
    if !config.exists() {
        return Default::default();
    }

    let config = match File::open(config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{}: Cannot open config file {}" , config.display(), error);
            return Default::default();
        }
    };

    match serde_json::from_reader(config) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Cannot parse config file: {}" , error);
            return Default::default();
        }
    }
}

struct App<'a> {
    config: path::PathBuf,
    stdout: io::StdoutLock<'a>,
    stdin: io::StdinLock<'a>,
    storage: sec_store::Store,
    is_changed: bool,
}

impl<'a> App<'a> {
    pub fn new(mut config: path::PathBuf, mut stdout: io::StdoutLock<'a>, mut stdin: io::StdinLock<'a>) -> Option<Self> {
        config.push(CONFIG);
        let data = read_config(config.as_path());

        let _ = stdout.write_all(b"Please enter your phrase\n");
        let mut password = String::new();
        match stdin.read_line(&mut password) {
            Ok(0) | Err(_) => {
                return None;
            },
            Ok(_) => {
                let trimmed = password.trim();
                if trimmed.len() == 0 {
                    return None;
                }

                trimmed.to_owned()
            },
        };

        let user_name = whoami::username();
        let mut storage = sec_store::Store::from_inner(data, user_name.as_bytes(), password.as_bytes());

        let is_changed = if storage.len() != 0 {
            let mut buf = [0u8; 128];
            //Password is invalid
            if let Err(_) = storage.get_to(VERSION_KEY.as_bytes(), &mut buf) {
                return None;
            }
            false
        } else {
            let _ = storage.insert(VERSION_KEY.as_bytes(), VERSION.as_bytes());
            true
        };

        Some(Self {
            config,
            stdout,
            stdin,
            storage,
            is_changed,
        })
    }

    #[inline]
    pub fn clear_screen(&mut self) {
        let _ = self.stdout.write_all(b"\x1B[2J\x1B[1;1H");
    }

    #[inline]
    pub fn help(&mut self) {
        let _ = self.stdout.write_fmt(format_args!("Usage:\n{}", HELP));
    }

    pub fn cmd_add<'c, T: Iterator<Item = &'c str>>(&mut self, mut args: T) {
        let label = match args.next() {
            Some(label) => if label.eq_ignore_ascii_case(VERSION_KEY) {
                let _ = self.stdout.write_all(b"Invalid <label>\n");
                return;
            } else {
                label
            },
            None => {
                let _ = self.stdout.write_all(b"Missing <label>\n");
                return;
            }
        };

        let data = match args.next() {
            Some(data) => data,
            None => {
                let _ = self.stdout.write_all(b"Missing <data>\n");
                return;
            }
        };

        let data = match data_encoding::BASE32_NOPAD.decode(data.as_bytes()) {
            Ok(data) => data,
            Err(_) => {
                let _ = self.stdout.write_all(b"<data> is not base32\n");
                return;
            }
        };

        self.storage.insert_owned(label.as_bytes(), data);
        self.is_changed = true;
    }

    pub fn cmd_show<'c, T: Iterator<Item = &'c str>>(&mut self, mut args: T) {
        let label = match args.next() {
            Some(label) => if label.eq_ignore_ascii_case(VERSION_KEY) {
                let _ = self.stdout.write_all(b"Invalid <label>\n");
                return;
            } else {
                label
            },
            None => {
                let _ = self.stdout.write_all(b"Missing <label>\n");
                return;
            }
        };

        let mut buffer = [0u8; 128];
        let otp = match self.storage.get_to(label.as_bytes(), &mut buffer) {
            Ok(0) | Err(_) => {
                let _ = self.stdout.write_all(b"Unknown <label>\n");
                return;
            }
            Ok(n) => otpshka::TOTP::new(otpshka::Algorithm::SHA1, &buffer[..n]),
        };

        otp.generate_to_now(&mut buffer[..6]);
        let pass = unsafe {
            core::str::from_utf8_unchecked(&buffer[..6])
        };
        self.clear_screen();
        let _ = self.stdout.write_fmt(format_args!("Pass: {}\n", pass));
    }

    pub fn cmd_remove<'c, T: Iterator<Item = &'c str>>(&mut self, mut args: T) {
        let label = match args.next() {
            Some(label) => if label.eq_ignore_ascii_case(VERSION_KEY) {
                let _ = self.stdout.write_all(b"Invalid <label>\n");
                return;
            } else {
                label
            },
            None => {
                let _ = self.stdout.write_all(b"Missing <label>\n");
                return;
            }
        };

        let _ = match self.storage.remove_key(label.as_bytes()) {
            true => self.stdout.write_all(b"Removed\n"),
            false => self.stdout.write_all(b"Unknown <label>\n"),
        };
    }

    pub fn run(&mut self) {
        let mut input = String::new();
        self.clear_screen();
        self.help();
        loop {
            input.truncate(0);
            let _ = self.stdout.write_all(b">");
            let _ = self.stdout.flush();
            match self.stdin.read_line(&mut input) {
                Ok(0) | Err(_) => {
                    continue;
                },
                _ => (),
            };

            let input = input.trim();
            let mut input_split = input.split_whitespace();

            let cmd = match input_split.next() {
                Some(cmd) => cmd,
                None => unsafe {
                    core::hint::unreachable_unchecked();
                }
            };

            if cmd.eq_ignore_ascii_case("add") {
                self.cmd_add(input_split)
            } else if cmd.eq_ignore_ascii_case("show") {
                self.cmd_show(input_split)
            } else if cmd.eq_ignore_ascii_case("remove") {
                self.cmd_remove(input_split)
            } else if cmd.eq_ignore_ascii_case("help") {
                self.clear_screen();
                self.help();
            } else if cmd.eq_ignore_ascii_case("exit") {
                self.clear_screen();
                return;
            } else {
                let _ = self.stdout.write_fmt(format_args!("Unknown command: '{}'\n", cmd));
            }
        }
    }
}

impl<'a> Drop for App<'a> {
    fn drop(&mut self) {
        if self.is_changed {
            let config = match File::create(self.config.as_path()) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("{}: Cannot open config file {}" , self.config.display(), error);
                    return;
                }
            };

            if let Err(error) = serde_json::to_writer_pretty(config, self.storage.inner()) {
                eprintln!("Cannot write config file: {}" , error);
            }
        }
    }
}

fn main() {
    let home = match std::env::current_exe() {
        Ok(mut home) => {
            home.pop();
            home
        },
        Err(_) => {
            eprintln!("Cannot get access to self directory");
            return;
        }
    };

    //Enable clearing console
    #[cfg(windows)]
    unsafe {
        use winapi::um::winbase::STD_OUTPUT_HANDLE;
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::processenv::GetStdHandle;
        use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
        use winapi::um::wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;

        let std_handle = GetStdHandle(STD_OUTPUT_HANDLE);

        if std_handle == INVALID_HANDLE_VALUE {
            return;
        }

        let mut current_mode = 0;

        if GetConsoleMode(std_handle, &mut current_mode) == 0 {
            return
        }

        SetConsoleMode(std_handle, current_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
    }

    let stdout = io::stdout();
    let stdin = io::stdin();
    match App::new(home, stdout.lock(), stdin.lock()) {
        Some(mut app) => app.run(),
        None => (),
    };
}
