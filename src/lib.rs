//! Embedded Menu System

//#![no_std]

#![deny(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    //warnings,
    //unused,
    unsafe_code,
)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::wildcard_dependencies
)]
#![allow(clippy::integer_arithmetic)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::toplevel_ref_arg)]
#![allow(clippy::print_stdout)]


#[cfg(test)]
#[macro_use] extern crate std;

#[cfg(test)]
#[macro_use] extern crate alloc;

use arraydeque::{behavior::Wrapping, ArrayDeque};
use embedded_hal::serial::{Read as HalRead, Write as HalWrite};
use heapless::{consts::U32, String};

mod macros;

const BACKSPACE: char = '\x08';
const NEWLINE: char = '\n';
const CARRIAGE_RETURN: char = '\r';

pub enum CallbackError {
    ParseError,
}
impl From<core::num::ParseIntError> for CallbackError {
    fn from(_error: core::num::ParseIntError) -> Self {
        CallbackError::ParseError
    }
}
impl From<core::num::ParseFloatError> for CallbackError {
    fn from(_error: core::num::ParseFloatError) -> Self {
        CallbackError::ParseError
    }
}
impl From<core::str::ParseBoolError> for CallbackError {
    fn from(_error: core::str::ParseBoolError) -> Self {
        CallbackError::ParseError
    }
}
impl From<core::char::ParseCharError> for CallbackError {
    fn from(_error: core::char::ParseCharError) -> Self {
        CallbackError::ParseError
    }
}

type ExecCallbackFn<C> = fn(context: &mut C);
type ReadCallbackFn<C> = fn(buf: &mut dyn core::fmt::Write, context: &C);
type WriteCallbackFn<C> = fn(arg: &String<U32>, context: &mut C) -> Result<(), CallbackError>;

#[allow(dead_code)]
pub enum MenuItemType<'a, C> {
    SubMenu(&'a [&'a MenuItem<'a, C>]),
    ReadValue(ReadCallbackFn<C>),
    WriteValue(ReadCallbackFn<C>, WriteCallbackFn<C>),
    ExecValue(ExecCallbackFn<C>),
}

pub struct MenuItem<'a, C> {
    pub name: &'a str,
    pub hint: Option<&'a str>,
    pub parent: Option<&'a MenuItem<'a, C>>,
    pub menu_type: MenuItemType<'a, C>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuState {
    Init,
    NeedIdx,
    NeedEnter,
    Processing,
}

#[allow(dead_code)]
pub struct Dispatcher<'a, Context> {
    state: MenuState,
    refresh_menu: bool,
    current_item: &'a MenuItem<'a, Context>,
    buffer: ArrayDeque<[char; 32], Wrapping>,
}

#[allow(dead_code)]
impl<'a, Context> Dispatcher<'a, Context> {
    pub fn new(main_menu: &'a MenuItem<'_, Context>) -> Self {
        Dispatcher {
            state: MenuState::Init,
            refresh_menu: false,
            current_item: main_menu,
            buffer: ArrayDeque::new(),
        }
    }

    pub const fn with_refresh(mut self) -> Self {
        self.refresh_menu = true;
        self
    }

    fn get_input<S, E>(&mut self, serial: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalRead<u8, Error = E> + HalWrite<u8, Error = E>,
    {
        while self.state != MenuState::Processing {
            let err = serial.read();
            if let Ok(c) = err {
                let c = c as char;

                match self.state {
                    MenuState::Init => {
                        self.state = MenuState::NeedIdx;
                    }
                    MenuState::NeedIdx => {
                        if c.is_ascii_hexdigit() {
                            let _ = self.buffer.push_back(c);
                            self.state = MenuState::Processing;
                        } else if c == BACKSPACE {
                            let _ = self.buffer.push_back('0');
                            self.state = MenuState::Processing;
                        } else if c == NEWLINE {
                            continue;
                        } else {
                            if c == 'x' {
                                #[cfg(test)]
                                return Err(nb::Error::WouldBlock);
                            }
                            continue;
                        }
                    }
                    MenuState::NeedEnter => {
                        if c == BACKSPACE {
                            let _ = self.buffer.pop_back();
                            sprint!(serial, "{} {}", BACKSPACE, BACKSPACE)?;
                        } else if c == NEWLINE || c == CARRIAGE_RETURN {
                            sprintln!(serial)?;
                            sprintln!(serial)?;
                            self.state = MenuState::Processing;
                        } else {
                            let _ = self.buffer.push_back(c);
                            sprint!(serial, "{}", c)?;
                        }
                    }
                    MenuState::Processing => {}
                }
            } else if let Err(e) = err {
                return Err(e);
            }
        }

        Ok(())
    }

    fn display_value<S, E>(&self, ctx: &Context, menu: &MenuItem<'a, Context>, tx: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalWrite<u8, Error = E>,
    {
        match menu.menu_type {
            MenuItemType::SubMenu(..) | MenuItemType::ExecValue(..) => (),
            MenuItemType::ReadValue(ref cb) | MenuItemType::WriteValue(ref cb, ..) => {
                let mut buffer = String::<U32>::new();
                cb(&mut buffer, ctx);
                sprint!(tx, ": {}", buffer)?;
            }
        }

        if let Some(hint) = menu.hint {
            sprintln!(tx, " ({})", hint)
        }
        else {
            sprintln!(tx)
        }
    }

    fn display_menu<S, E>(&self, ctx: &mut Context, tx: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalWrite<u8, Error = E>,
    {
        sprintln!(tx, "{}", self.current_item.name)?;

        if let MenuItemType::SubMenu(children) = self.current_item.menu_type {
            if let Some(parent) = self.current_item.parent {
                sprintln!(tx, " <0> --> {}", parent.name)?;
            }

            for (i, c) in children.iter().enumerate() {
                match c.menu_type {
                    MenuItemType::SubMenu(..)    => { sprint!(tx, " [{}] --> {}", i+1, c.name)?; },
                    MenuItemType::ReadValue(..)  => { sprint!(tx, " [{}] r=> {}", i+1, c.name)?; },
                    MenuItemType::WriteValue(..) => { sprint!(tx, " [{}] w=> {}", i+1, c.name)?; },
                    MenuItemType::ExecValue(..)  => { sprint!(tx, " [{}] e=> {}", i+1, c.name)?; },
                }

                self.display_value(ctx, c, tx)?;
            }

            sprintln!(tx)?;
        }

        Ok(())
    }

    pub fn run<S, E>(&mut self, ctx: &mut Context, serial: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalRead<u8, Error = E> + HalWrite<u8, Error = E>,
    {
        let mut result = Ok(());

        if self.state == MenuState::Init {
            self.display_menu(ctx, serial)?;
            self.state = MenuState::NeedIdx;
        }

        while result.is_ok() {
            let mut changed = false;

            self.get_input(serial)?;

            if let Some(input) = self.buffer.front() {
                let current_idx = if let Some(digit) = input.to_digit(16) {
                    digit as usize
                } else {
                    let _ = self.buffer.pop_front();
                    continue;
                };

                result = if current_idx == 0 {
                    self.state = MenuState::NeedIdx;
                    if let Some(parent) = self.current_item.parent {
                        let _ = self.buffer.pop_front();
                        self.current_item = parent;
                        changed = true;
                        Ok(())
                    } else {
                        let _ = self.buffer.pop_front();
                        Ok( () )
                    }
                } else if let MenuItemType::SubMenu(menu) = self.current_item.menu_type {
                    self.state = MenuState::NeedIdx;
                    let _ = self.buffer.pop_front();

                    if let Some(child) = menu.get(current_idx - 1) {
                        match child.menu_type {
                            MenuItemType::SubMenu(_) => {
                                self.current_item = child;
                                changed = true;
                                Ok(())
                            }
                            MenuItemType::ReadValue(rcb) => {
                                let mut buffer = String::<U32>::new();
                                rcb(&mut buffer, ctx);
                                sprintln!(serial, "{}: {}", child.name, buffer)?;
                                sprintln!(serial)
                            }
                            MenuItemType::ExecValue(ecb) => {
                                ecb(ctx);
                                Ok( () )
                            }
                            MenuItemType::WriteValue(rcb, _) => {
                                // TODO: Input
                                self.state = MenuState::NeedEnter;
                                self.current_item = child;

                                let mut buffer = String::<U32>::new();
                                rcb(&mut buffer, ctx);
                                sprintln!(serial, "{}: {}", child.name, buffer)?;
                                sprintln!(serial, "Enter new value:")
                            }
                        }
                    } else {
                        Ok( () )
                    }
                } else if let MenuItemType::WriteValue(_rcb, wcb) = self.current_item.menu_type {
                    let mut buffer = String::<U32>::new();

                    /* Set menu state after writing */
                    self.state = MenuState::NeedIdx;
                    self.current_item = self.current_item.parent.expect("Item has no parent?");
                    changed = true;

                    for i in 0..buffer.capacity() {
                        if let Some(c) = self.buffer.pop_front() {
                            if i < buffer.capacity() {
                                buffer.push(c).expect("cannot push into buffer?!?");
                            }
                        }
                        else { break; }
                    }

                    /* Clean input buffer if some joker put a lot in it. */
                    while let Some(_) = self.buffer.pop_front() {}

                    if wcb(&buffer, ctx).is_err() {
                        sprintln!(serial, "Unable to parse {}", buffer)?;
                    }

                    Ok( () )
                } else {
                    unimplemented!();
                };
            };

            if changed {
                self.display_menu(ctx, serial)?;
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use embedded_hal_mock as mock;
    use mock::serial::{Mock as SerialMock, Transaction as SerialTransaction};

    struct Context {
        bool_value: bool,
        uint_value: u32,
    }

    mitem!(Main,
           MAIN_MENU = "Main Menu",
           [&SUB1, &SUB2]
    );

    mitem!(Read (&SUB1),
           BOOL_VAL = "bool", ("boolean")
           r=> |buf, ctx| { let _ = write!(buf, "{}", ctx.bool_value); }
    );

    mitem!(Read (&SUB2),
           UINT_VAL = "Uint",
           r=> |buf, ctx| { let _ = write!(buf, "{}", ctx.uint_value); }
    );

    static UINT_VAL_WRITE: MenuItem<'_, Context> = MenuItem {
        name: "Uint_Write",
        hint: None,
        parent: Some(&SUB2),
        menu_type: MenuItemType::WriteValue(|buf, ctx| {
            let _ = write!(buf, "{}", ctx.uint_value);
        },
        |buf, ctx| {
            let i: u32 = buf.parse()?;
            ctx.uint_value = i;
            Ok( () )
        }),
    };

    static SUB1: MenuItem<'_, Context> = MenuItem {
        name: "Sub Menu 1",
        hint: None,
        parent: Some(&MAIN_MENU),
        menu_type: MenuItemType::SubMenu(&[&BOOL_VAL])
    };

    static SUB2: MenuItem<'_, Context> = MenuItem {
        name: "Sub Menu 2",
        hint: None,
        parent: Some(&MAIN_MENU),
        menu_type: MenuItemType::SubMenu(&[&UINT_VAL, &UINT_VAL_WRITE])
    };

    #[test]
    fn simple() {
        let mut context = Context { bool_value: true, uint_value: 32 };

        let mut runner = Dispatcher::new(&MAIN_MENU).with_refresh();

        // Configure expectations
        let expectations = [
            // Printing Main Menu
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Sub Menu 1
            SerialTransaction::read(b'1'),
            SerialTransaction::write_many(&format!("{}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> -->  {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {} ({})\r\n", BOOL_VAL.name, context.bool_value, BOOL_VAL.hint.unwrap() )),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Main Menu
            SerialTransaction::read(b'0'),
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many(&format!("{}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> --> {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {}\r\n", UINT_VAL.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] w=> {}: {}\r\n", UINT_VAL_WRITE.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Show UINT_VAL
            SerialTransaction::read(b'1'),
            SerialTransaction::write_many(&format!("{}: {}\r\n", UINT_VAL.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::read(b'\x08'),
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // End Test
            SerialTransaction::read(b'x'),
        ];

        let mut serial = SerialMock::new(&expectations);

        match runner.run(&mut context, &mut serial) {
            Ok(_) => {}
            Err(e1) => {
                if e1 != nb::Error::WouldBlock {
                    panic!("Error: {:?}", e1);
                }
            }
        }
    }

    #[test]
    fn input() {
        let mut context = Context { bool_value: true, uint_value: 32 };

        let mut runner = Dispatcher::new(&MAIN_MENU).with_refresh();

        // Configure expectations
        let expectations = [
            // Printing Main Menu
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many(&format!("{}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> --> {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {}\r\n", UINT_VAL.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] w=> {}: {}\r\n", UINT_VAL_WRITE.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Editting UINT_VAL_WRITE
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many(&format!("{}: {}\r\n", UINT_VAL_WRITE.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(b"Enter new value:\r\n"),
            SerialTransaction::flush(),

            // Input
            SerialTransaction::read(b'3'),
            SerialTransaction::write_many("3"),
            SerialTransaction::read(b'\x08'), // backspace
            SerialTransaction::write_many("\x08 \x08"),
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many("2"),
            SerialTransaction::read(b'5'),
            SerialTransaction::write_many("5"),
            SerialTransaction::read(b'\n'),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::write_many(&format!("{}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> --> {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {}\r\n", UINT_VAL.name, 25)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] w=> {}: {}\r\n", UINT_VAL_WRITE.name, 25)),
            SerialTransaction::flush(),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),

            // End Test
            SerialTransaction::read(b'x'),
        ];

        let mut serial = SerialMock::new(&expectations);

        match runner.run(&mut context, &mut serial) {
            Ok(_) => {}
            Err(e1) => {
                if e1 != nb::Error::WouldBlock {
                    panic!("Error: {:?}", e1);
                }
            }
        }
    }
}
