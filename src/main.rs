use std::io::IsTerminal;

use connectui::ui::app::FullScreen;
use iocraft::prelude::*;

fn main() {
    if !std::io::stdout().is_terminal() {
        panic!("Please open the application in a terminal window.");
    }
    smol::block_on(element!(FullScreen).fullscreen()).unwrap();
}
