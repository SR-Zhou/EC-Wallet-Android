use dioxus::prelude::*;

fn main()
{
    dioxus::launch(gui::app);
}

mod blockchain;
mod crypto;
mod gui;