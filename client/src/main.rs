// A command line chat client will probably need some styling / high-level interaction with the command line. So this
// project includes `crossterm` by default, a Rust library for cross-plattform command line manipulation

use crossterm::{
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};

fn main() {
    // crossterm commands can fail. For now, you are allowed to use `.unwrap()`, but you can also take a look at error
    // handling in Rust and do it right from the start. We will talk about error handling probably towards the end of
    // May

    std::io::stdout()
        .execute(SetForegroundColor(Color::Blue))
        .unwrap()
        .execute(SetBackgroundColor(Color::Red))
        .unwrap()
        .execute(Print("Styled text here."))
        .unwrap()
        .execute(ResetColor)
        .unwrap();
}
