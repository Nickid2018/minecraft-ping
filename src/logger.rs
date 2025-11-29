use clap::ValueEnum;
use colored::Colorize;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, ValueEnum)]
pub enum LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
    QUIET,
}

struct SimpleLogger {
    level: LogLevel,
    no_color: bool,
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let meta_level = if metadata.target().starts_with("fast_socks5") {
            Level::Trace
        } else {
            metadata.level()
        };
        match self.level {
            LogLevel::TRACE => true,
            LogLevel::DEBUG => meta_level <= Level::Debug,
            LogLevel::INFO => meta_level <= Level::Info,
            LogLevel::WARN => meta_level <= Level::Warn,
            LogLevel::ERROR => meta_level <= Level::Error,
            LogLevel::QUIET => false,
        }
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let output_level = if record.target().starts_with("fast_socks5") {
                Level::Trace
            } else {
                record.level()
            };
            let print = match self.level {
                LogLevel::TRACE | LogLevel::DEBUG => {
                    format!("[{}] {}", record.target().replace("::", "/"), record.args())
                }
                _ => format!("{}", record.args()),
            };
            let colored = if self.no_color {
                print.white().clear()
            } else {
                match output_level {
                    Level::Trace => print.bright_black(),
                    Level::Debug => print.magenta(),
                    Level::Info => print.white().clear(),
                    Level::Warn => print.bright_yellow(),
                    Level::Error => print.red(),
                }
            };
            println!("{}", colored);
        }
    }

    fn flush(&self) {}
}

pub fn init(level: LogLevel, no_color: bool) -> Result<(), SetLoggerError> {
    log::set_boxed_logger(Box::new(SimpleLogger { level, no_color }))
        .map(|()| log::set_max_level(LevelFilter::Trace))
}
