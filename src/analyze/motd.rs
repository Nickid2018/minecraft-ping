use crate::analyze::{Analyzer, AvailableAnalyzers, MotdInfo, StatusPayload};
use crate::logger::LogLevel;
use crate::mode::QueryMode;
use async_trait::async_trait;
use clap::Args;
use colored::{ColoredString, Colorize};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::LazyLock;

pub struct Motd<'a> {
    args: &'a MotdArgs,
}

#[derive(Args, Debug)]
pub struct MotdArgs {
    /// Do not stylize motd, only output the display string
    #[arg(long)]
    pub no_motd_styles: bool,
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
    let motd = &mut args.analyzer_args.motd_args;
    if args.no_color {
        motd.no_motd_styles = true;
    }
    if !motd.no_motd_true_colors && !motd.no_motd_styles && !motd.raw_motd {
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

static JAVA_NAME_TO_CHAR: LazyLock<HashMap<&str, char>> = LazyLock::new(|| {
    let mut output = HashMap::new();
    output.insert("black", '0');
    output.insert("dark_blue", '1');
    output.insert("dark_green", '2');
    output.insert("dark_aqua", '3');
    output.insert("dark_red", '4');
    output.insert("dark_purple", '5');
    output.insert("gold", '6');
    output.insert("gray", '7');
    output.insert("dark_gray", '8');
    output.insert("blue", '9');
    output.insert("green", 'a');
    output.insert("aqua", 'b');
    output.insert("red", 'c');
    output.insert("light_purple", 'd');
    output.insert("yellow", 'e');
    output.insert("white", 'f');
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

fn str_to_chars(str: &str) -> Vec<char> {
    str.chars().collect::<Vec<char>>()
}

fn try_get<'a>(obj: &'a Map<String, Value>, key: &str) -> &'a Value {
    obj.get(key).unwrap_or(&Value::Null)
}

fn make_text_component(
    component: &Value,
    base_style: &ColoredString,
    true_color: bool,
) -> Vec<ColoredString> {
    if component.is_array() {
        return component
            .as_array()
            .expect("Should be array")
            .iter()
            .map(|c| make_text_component(c, base_style, true_color))
            .flatten()
            .collect();
    }

    if component.is_string() {
        return vec![copy_style(
            base_style,
            &str_to_chars(component.as_str().expect("Should be string"))[..],
        )];
    }

    let object = component.as_object().expect("Should be object");
    let mut my_style = copy_style(
        base_style,
        &str_to_chars(try_get(object, "text").as_str().unwrap_or(""))[..],
    );

    if let Some(color) = try_get(object, "color").as_str() {
        if !color.starts_with("#") {
            let ch = JAVA_NAME_TO_CHAR.get(color).unwrap_or(&'?');
            if let Some(style) = JAVA_INSTRUCTIONS.get(ch) {
                if true_color && let Some(color) = style.0 {
                    my_style = my_style.truecolor(color.0, color.1, color.2);
                } else {
                    my_style = style.1(my_style);
                }
            } else {
                log::warn!("Invalid color string: {}", color);
            }
        } else if true_color {
            if color.is_ascii() && color.len() == 7 {
                my_style = my_style.truecolor(
                    color[1..3].parse().unwrap_or(0),
                    color[3..5].parse().unwrap_or(0),
                    color[5..7].parse().unwrap_or(0),
                );
            } else {
                log::warn!("Invalid color string: {}", color);
                my_style = my_style.red();
            }
        } else {
            my_style = my_style.white();
        }
    }

    if true_color && let Some(color) = try_get(object, "shadow_color").as_u64() {
        // ARGB
        my_style = my_style.on_truecolor(
            (color >> 16 & 0xFF) as u8,
            (color >> 8 & 0xFF) as u8,
            (color & 0xFF) as u8,
        );
    }

    if try_get(object, "bold").as_bool().unwrap_or(false) {
        my_style = my_style.bold();
    }
    if try_get(object, "italic").as_bool().unwrap_or(false) {
        my_style = my_style.italic();
    }
    if try_get(object, "underlined").as_bool().unwrap_or(false) {
        my_style = my_style.underline();
    }
    if try_get(object, "strikethrough").as_bool().unwrap_or(false) {
        my_style = my_style.strikethrough();
    }
    if try_get(object, "obfuscated").as_bool().unwrap_or(false) {
        my_style = my_style.hidden();
    }

    let extra = object
        .get("extra")
        .map(|e| make_text_component(e, &my_style, true_color));
    let mut subs = vec![my_style];
    extra.map(|extra| subs.extend(extra));
    subs
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
                } else if self.args.no_motd_styles {
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
                if self.args.raw_motd {
                    log::info!("{}", s);
                    return;
                }
                let true_color = !self.args.no_motd_true_colors;
                let texts = make_text_component(s, &default_style(true_color), true_color);
                if self.args.no_motd_styles {
                    texts.iter().for_each(|s| print!("{}", s.input));
                } else {
                    texts.iter().for_each(|s| print!("{}", s));
                }
                println!();
            }
        }
    }
}

impl Motd<'_> {
    pub fn new(args: &'_ MotdArgs) -> Motd<'_> {
        Motd { args }
    }
}
