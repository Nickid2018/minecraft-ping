use crate::analyze::{Analyzer, AvailableAnalyzers, MotdInfo, StatusPayload};
use crate::logger::LogLevel;
use crate::mode::QueryMode;
use async_trait::async_trait;
use clap::Args;
use colored::{ColoredString, Colorize};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub struct Motd<'a> {
    args: &'a MotdArgs,
}

#[derive(Args, Debug)]
pub struct MotdArgs {
    /// Do not colorize motd
    #[arg(long)]
    pub no_motd_colors: bool,
    /// Display motd in terminal colors instead of true colors
    #[arg(long)]
    pub no_motd_true_colors: bool,
    /// Do not parse MOTD strings and output them in raw strings
    #[arg(long)]
    pub raw_motd: bool,
}

pub fn sanitize_motd_args(args: &mut crate::BaseArgs) {
    if args.log_level > LogLevel::INFO {
        args.analyzer_args
            .analyzers
            .pop_if(|i| *i == AvailableAnalyzers::MOTD);
        return;
    }
    let motd = &mut args.analyzer_args.motd;
    if args.no_color {
        motd.no_motd_colors = true;
    }
    if !motd.no_motd_true_colors && !motd.no_motd_colors && !motd.raw_motd {
        if let Some(term) = std::env::var("COLORTERM").ok() {
            if term != "truecolor" && term != "24bit" {
                log::warn!(
                    "Terminal doesn't support true colors, MOTD will use ANSI colors ($COLORTERM = {})",
                    term
                );
                motd.no_motd_true_colors = true;
            }
        }
    }
}

fn default_style(true_color: bool) -> ColoredString {
    if true_color {
        "".truecolor(170, 170, 170)
    } else {
        "".white()
    }
}

type TrueColor = (u8, u8, u8);
type Formatter = fn(ColoredString) -> ColoredString;

static MOTD_COMMONS: LazyLock<HashMap<char, (Option<TrueColor>, Formatter)>> =
    LazyLock::new(|| {
        let mut commons: HashMap<char, (Option<TrueColor>, Formatter)> = HashMap::new();
        commons.insert('0', (Some((0, 0, 0)), |s| s.black()));
        commons.insert('1', (Some((0, 0, 170)), |s| s.blue()));
        commons.insert('2', (Some((0, 170, 0)), |s| s.green()));
        commons.insert('3', (Some((0, 170, 170)), |s| s.cyan()));
        commons.insert('4', (Some((170, 0, 0)), |s| s.red()));
        commons.insert('5', (Some((170, 0, 170)), |s| s.purple()));
        commons.insert('6', (Some((255, 170, 0)), |s| s.yellow()));
        commons.insert('7', (Some((170, 170, 170)), |s| s.white()));
        commons.insert('8', (Some((85, 85, 85)), |s| s.bright_black()));
        commons.insert('9', (Some((85, 85, 255)), |s| s.bright_blue()));
        commons.insert('a', (Some((85, 255, 85)), |s| s.bright_green()));
        commons.insert('b', (Some((85, 255, 255)), |s| s.bright_cyan()));
        commons.insert('c', (Some((255, 85, 85)), |s| s.bright_red()));
        commons.insert('d', (Some((255, 85, 255)), |s| s.bright_purple()));
        commons.insert('e', (Some((255, 255, 85)), |s| s.bright_yellow()));
        commons.insert('f', (Some((255, 255, 255)), |s| s.bright_white()));
        commons.insert('k', (None, |s| s.hidden()));
        commons.insert('l', (None, |s| s.bold()));
        commons.insert('o', (None, |s| s.italic()));
        commons
    });

static JAVA_INSTRUCTIONS: LazyLock<HashMap<char, (Option<TrueColor>, Formatter)>> =
    LazyLock::new(|| {
        let mut output = HashMap::new();
        MOTD_COMMONS.iter().for_each(|(k, v)| {
            output.insert(*k, *v);
        });
        output.insert('m', (None, |s| s.strikethrough()));
        output.insert('n', (None, |s| s.underline()));
        output
    });

static BEDROCK_INSTRUCTIONS: LazyLock<HashMap<char, (Option<TrueColor>, Formatter)>> =
    LazyLock::new(|| {
        let mut extra: HashMap<char, TrueColor> = HashMap::new();
        extra.insert('g', (221, 214, 5));
        extra.insert('h', (227, 212, 209));
        extra.insert('i', (206, 202, 202));
        extra.insert('j', (68, 58, 59));
        extra.insert('m', (151, 22, 7));
        extra.insert('n', (180, 104, 77));
        extra.insert('p', (222, 177, 45));
        extra.insert('q', (17, 160, 54));
        extra.insert('s', (44, 186, 168));
        extra.insert('t', (33, 73, 123));
        extra.insert('u', (154, 92, 198));
        extra.insert('v', (235, 114, 20));
        let mut output = HashMap::new();
        MOTD_COMMONS.iter().for_each(|(k, v)| {
            output.insert(*k, *v);
        });
        extra.into_iter().for_each(|(k, v)| {
            output.insert(k, (Some(v), |s| s));
        });
        output
    });

fn no_color_motd_string(str: &str) {
    log::info!(
        "{}",
        Regex::new("§.")
            .expect("Could not compile regex")
            .replace_all(str, "")
    );
}

fn copy_style(style: &ColoredString, chars: &[char]) -> ColoredString {
    let mut make = chars.iter().collect::<String>().replace("§§", "§").normal();
    make.bgcolor = style.bgcolor;
    make.fgcolor = style.fgcolor;
    make.style = style.style;
    make
}

fn color_motd_string(str: &str, be: bool, true_color: bool) {
    let mut last_style = default_style(true_color);

    let chars: Vec<char> = str.chars().collect();
    let mut last_index = 0;
    let mut left_index = 0;

    let mut slices = Vec::new();
    while let Some(next_sec) = chars[left_index..].iter().position(|c| *c == '§') {
        let format_opt = chars.get(left_index + next_sec + 1);
        if format_opt.is_none() {
            break;
        }
        let format = *format_opt.expect("format is none");

        if format == '§' {
            left_index += next_sec + 2;
            continue;
        }

        slices.push(copy_style(
            &last_style,
            &chars[last_index..left_index + next_sec],
        ));

        match format {
            'r' => last_style = default_style(true_color),
            _ => {
                let ins = match be {
                    true => &BEDROCK_INSTRUCTIONS,
                    false => &JAVA_INSTRUCTIONS,
                };
                if let Some(c) = ins.get(&format) {
                    if true_color && let Some(color) = c.0 {
                        last_style = last_style.truecolor(color.0, color.1, color.2);
                    } else {
                        last_style = c.1(last_style);
                    }
                }
            }
        }
        left_index += next_sec + 2;
        last_index = left_index;
    }

    slices.push(copy_style(&last_style, &chars[last_index..]));
    slices.iter().for_each(|slice| print!("{}", slice));
    println!();
}

#[async_trait]
impl Analyzer for Motd<'_> {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        payload.motd.is_some()
    }

    async fn analyze(&self, payload: &StatusPayload) {
        let motd = payload.motd.as_ref().expect("No motd found");
        match motd {
            MotdInfo::String(motd_string) => {
                if self.args.raw_motd {
                    log::info!("{}", motd_string);
                } else if self.args.no_motd_colors {
                    no_color_motd_string(motd_string);
                } else {
                    color_motd_string(
                        motd_string,
                        payload.mode == QueryMode::BEDROCK,
                        !self.args.no_motd_true_colors,
                    );
                }
            }
            MotdInfo::Component(s) => {
                log::info!("{}", s.to_string());
            }
        }
    }
}

impl Motd<'_> {
    pub fn new(args: &'_ MotdArgs) -> Motd<'_> {
        Motd { args }
    }
}
