use clap::ValueEnum;
use colored::Colorize;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

#[derive(Debug, Copy, Clone, ValueEnum)]
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
        match self.level {
            LogLevel::TRACE => true,
            LogLevel::DEBUG => metadata.level() <= Level::Debug,
            LogLevel::INFO => metadata.level() <= Level::Info,
            LogLevel::WARN => metadata.level() <= Level::Warn,
            LogLevel::ERROR => metadata.level() <= Level::Error,
            LogLevel::QUIET => false,
        }
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let print = match self.level {
                LogLevel::TRACE | LogLevel::DEBUG => {
                    format!("[{}] {}", record.target().replace("::", "/"), record.args())
                }
                _ => format!("{}", record.args()),
            };
            let colored = if self.no_color {
                print.white().clear()
            } else {
                match record.level() {
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
