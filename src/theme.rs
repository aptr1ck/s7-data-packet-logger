use floem::prelude::Color;
use std::sync::RwLock;
use once_cell::sync::Lazy;

#[derive(Clone, Copy, Debug)]
pub enum Theme {
    Light,
    Dark,
}

pub static CURRENT_THEME: Lazy<RwLock<Theme>> = Lazy::new(|| RwLock::new(Theme::Light));

// Define the missing color constants if you want to use the global_text_color function
pub const COLOR_LIGHT: Color = Color::from_rgb8(101, 123, 131); // base00 for light theme text
pub const COLOR_DARK: Color = Color::from_rgb8(131, 148, 150);  // base0 for dark theme text

pub fn global_text_color() -> Color {
    match CURRENT_THEME.read().unwrap().clone() {
        Theme::Light => COLOR_LIGHT,
        Theme::Dark => COLOR_DARK,
    }
}

pub fn set_theme(theme: Theme) {
    *CURRENT_THEME.write().unwrap() = theme;
}

pub fn get_current_theme() -> Theme {
    CURRENT_THEME.read().unwrap().clone()
}

pub fn solarized_base0() -> Color {
     Color::from_rgb8(131, 148, 150) // base0
}

pub fn solarized_base00() -> Color {
     Color::from_rgb8(101, 123, 131) // base00
}

pub fn solarized_base1() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(147, 161, 161), // base1
        Theme::Dark => Color::from_rgb8(88, 110, 117),   // base01
    }
}

pub fn solarized_base01() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(88, 110, 117),   // base01
        Theme::Dark => Color::from_rgb8(147, 161, 161), // base1
    }
}

pub fn solarized_base1_8() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(211, 203, 183), // base1.8
        Theme::Dark => Color::from_rgb8(7, 54, 66),     // base02 TODO
    }
}

pub fn solarized_base1_9() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(217, 210, 194), // base1.9
        Theme::Dark => Color::from_rgb8(7, 54, 66),     // base02 TODO
    }
}

pub fn solarized_base2() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(238, 232, 213), // base2
        Theme::Dark => Color::from_rgb8(7, 54, 66),     // base02
    }
}

pub fn solarized_base02() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(7, 54, 66),     // base02
        Theme::Dark => Color::from_rgb8(238, 232, 213), // base2
    }
}

pub fn solarized_base3() -> Color {
    match get_current_theme() {
        Theme::Light => Color::from_rgb8(253, 246, 227), // base3
        Theme::Dark => Color::from_rgb8(0, 43, 54),     // base03
    }
}

// Additional Solarized accent colors (these are the same in both themes)
pub fn solarized_yellow() -> Color {
    Color::from_rgb8(181, 137, 0) // #b58900
}

pub fn solarized_orange() -> Color {
    Color::from_rgb8(203, 75, 22) // #cb4b16
}

pub fn solarized_red() -> Color {
    Color::from_rgb8(220, 50, 47) // #dc322f
}

pub fn solarized_magenta() -> Color {
    Color::from_rgb8(211, 54, 130) // #d33682
}

pub fn solarized_violet() -> Color {
    Color::from_rgb8(108, 113, 196) // #6c71c4
}

pub fn solarized_blue() -> Color {
    Color::from_rgb8(38, 139, 210) // #268bd2
}

pub fn solarized_cyan() -> Color {
    Color::from_rgb8(42, 161, 152) // #2aa198
}

pub fn solarized_green() -> Color {
    Color::from_rgb8(133, 153, 0) // #859900
}

/*
SOLARIZED HEX     16/8 TERMCOL  XTERM/HEX   L*A*B      RGB         HSB
--------- ------- ---- -------  ----------- ---------- ----------- -----------
base03    #002b36  8/4 brblack  234 #1c1c1c 15 -12 -12   0  43  54 193 100  21
base02    #073642  0/4 black    235 #262626 20 -12 -12   7  54  66 192  90  26
base01    #586e75 10/7 brgreen  240 #585858 45 -07 -07  88 110 117 194  25  46
base00    #657b83 11/7 bryellow 241 #626262 50 -07 -07 101 123 131 195  23  51
base0     #839496 12/6 brblue   244 #808080 60 -06 -03 131 148 150 186  13  59
base1     #93a1a1 14/4 brcyan   245 #8a8a8a 65 -05 -02 147 161 161 180   9  63
base2     #eee8d5  7/7 white    254 #e4e4e4 92 -00  10 238 232 213  44  11  93
base3     #fdf6e3 15/7 brwhite  230 #ffffd7 97  00  10 253 246 227  44  10  99
yellow    #b58900  3/3 yellow   136 #af8700 60  10  65 181 137   0  45 100  71
orange    #cb4b16  9/3 brred    166 #d75f00 50  50  55 203  75  22  18  89  80
red       #dc322f  1/1 red      160 #d70000 50  65  45 220  50  47   1  79  86
magenta   #d33682  5/5 magenta  125 #af005f 50  65 -05 211  54 130 331  74  83
violet    #6c71c4 13/5 brmagenta 61 #5f5faf 50  15 -45 108 113 196 237  45  77
blue      #268bd2  4/4 blue      33 #0087ff 55 -10 -45  38 139 210 205  82  82
cyan      #2aa198  6/6 cyan      37 #00afaf 60 -35 -05  42 161 152 175  74  63
green     #859900  2/2 green     64 #5f8700 60 -20  65 133 153   0  68 100  60
*/