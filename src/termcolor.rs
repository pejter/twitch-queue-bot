// Single-file console text color formatting
// http://www.lihaoyi.com/post/BuildyourownCommandLinewithANSIescapecodes.html
pub enum Color {
    Black,
    BgBlack,
    Red,
    BgRed,
    Green,
    BgGreen,
    Yellow,
    BgYellow,
    Blue,
    BgBlue,
    Magenta,
    BgMagenta,
    Purple,
    BgPurple,
    Cyan,
    BgCyan,
    White,
    BgWhite,
}

pub fn get_code(color: &Color) -> &str {
    match color {
        Color::Black => "30;1m",
        Color::BgBlack => "40;1m",

        Color::Red => "31;1m",
        Color::BgRed => "41;1m",

        Color::Green => "32;1m",
        Color::BgGreen => "42;1m",

        Color::Yellow => "33m",
        Color::BgYellow => "43;1m",

        Color::Blue => "34;1m",
        Color::BgBlue => "44;1m",

        Color::Magenta | Color::Purple => "35;1m",
        Color::BgMagenta | Color::BgPurple => "45;1m",

        Color::Cyan => "36;1m",
        Color::BgCyan => "46;1m",

        Color::White => "37;1m",
        Color::BgWhite => "47;1m",
    }
}

#[macro_export]
macro_rules! color {
    ($color : expr, $text:expr) => {{
        let code = crate::termcolor::get_code(&$color);
        format_args!("\u{001b}[{}{}\u{001b}[0m",code.to_owned(),$text)
    }};
    ($color : expr, $fmt:expr, $($args : tt) *) => {{
        let code = crate::termcolor::get_code(&$color);
        format_args!("\u{001b}[{}{}\u{001b}[0m",code.to_owned(),format_args!($fmt, $($args)*))
    }};
}

#[macro_export]
macro_rules! colorprintln {
    ($color : expr, $text:expr) => {{
        std::io::_print(color!($color,$text))
    }};
    ($color : expr, $fmt:expr, $($args : tt) *) => {{
        std::io::_print(color!($color , $fmt, $($args)*))
    }};
}
