//! Embedded Menu System

#![no_std]

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

use core::fmt::Write;
use arraydeque::{behavior::Wrapping, ArrayDeque};
use embedded_hal::serial::{Read as HalRead, Write as HalWrite};
use heapless::{
    consts::{
        U32, U64,
    },
    String
};

mod macros;

const DEL: char = '\x7f';
const BACKSPACE: char = '\x08';
const NEWLINE: char = '\n';
const CARRIAGE_RETURN: char = '\r';

type Word = u8;

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

type ActiveCallbackFn<C> = fn(context: &C) -> bool;
type ExecCallbackFn<C> = fn(context: &mut C);
type ReadCallbackFn<C> = fn(buf: &mut dyn Write, context: &C);
type WriteCallbackFn<C> = fn(arg: &String<U32>, context: &mut C) -> Result<(), CallbackError>;

#[allow(dead_code)]
pub enum MenuItemType<'a, C> {
    SubMenu(&'a [&'a MenuItem<'a, C>], ActiveCallbackFn<C>),
    ReadValue(ReadCallbackFn<C>),
    WriteValue(ReadCallbackFn<C>, WriteCallbackFn<C>),
    ExecValue(ReadCallbackFn<C>, ExecCallbackFn<C>),
}

pub struct MenuItem<'a, Context> {
    pub name: &'a str,
    pub hint: Option<&'a str>,
    pub parent: Option<&'a MenuItem<'a, Context>>,
    pub menu_type: MenuItemType<'a, Context>,
}
impl<'a, Context> MenuItem<'a, Context> {
    fn value_to_string<S, E>(&self, ctx: &mut Context, tx: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalWrite<Word, Error = E>
    {
        let mut string = String::<U32>::new();
        match self.menu_type {
            MenuItemType::SubMenu(..) => (),
            MenuItemType::ReadValue(ref rcb) | MenuItemType::ExecValue(ref rcb, ..) | MenuItemType::WriteValue(ref rcb, ..) => {
                rcb(&mut string, ctx);
                sprint!(tx, ": {}", string)?;
            }
        }

        Ok( () )
    }

    fn menu_item_to_string<S, E>(&self, idx: usize, ctx: &mut Context, tx: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalWrite<Word, Error = E>
    {
        match self.menu_type {
            MenuItemType::SubMenu(_, is_active)    => {
                sprint!(tx, " [{}] --> {}", idx, self.name)?;
                if !is_active(ctx) { sprint!(tx, " (Disabled)")?; }
            },
            MenuItemType::ReadValue(..)  => { sprint!(tx, " [{:X}] r=> {}", idx, self.name)?; self.value_to_string(ctx, tx)?; },
            MenuItemType::WriteValue(..) => { sprint!(tx, " [{:X}] w=> {}", idx, self.name)?; self.value_to_string(ctx, tx)?; },
            MenuItemType::ExecValue(..)  => { sprint!(tx, " [{:X}] e=> {}", idx, self.name)?; self.value_to_string(ctx, tx)?; },
        }

        if let Some(hint) = self.hint {
            sprint!(tx, " [{}]", hint)?;
        }

        sprintln!(tx)?;

        Ok( () )
    }
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

    pub const fn without_init(mut self) -> Self {
        self.state = MenuState::NeedIdx;
        self
    }

    fn get_input<S, E>(&mut self, serial: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalRead<Word, Error = E> + HalWrite<Word, Error = E>
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
                        } else if c == BACKSPACE || c == DEL {
                            let _ = self.buffer.push_back('0');
                            self.state = MenuState::Processing;
                        } else if c == NEWLINE || c == CARRIAGE_RETURN {
                            let _ = self.buffer.push_back(NEWLINE);
                            return Ok(());
                        } else {
                            if c == 'x' {
                                #[cfg(test)]
                                return Err(nb::Error::WouldBlock);
                            }
                            continue;
                        }
                    }
                    MenuState::NeedEnter => {
                        if c == BACKSPACE || c == DEL {
                            if self.buffer.pop_back().is_some() {
                                serial.write(BACKSPACE as u8)?;
                                serial.write(b' ')?;
                                serial.write(BACKSPACE as u8)?;
                                serial.flush()?;
                            }
                        } else if c == NEWLINE || c == CARRIAGE_RETURN {
                            sprintln!(serial)?;
                            if !self.buffer.is_empty() {
                                self.state = MenuState::Processing;
                            }
                        } else {
                            let _ = self.buffer.push_back(c);
                            serial.write(c as u8)?;
                            serial.flush()?;
                            serial.flush()?;
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

    fn display_menu<S, E>(&self, ctx: &mut Context, tx: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalWrite<Word, Error = E>
    {
        sprintln!(tx)?;
        sprintln!(tx, "{}", self.current_item.name)?;

        if let MenuItemType::SubMenu(children, is_active) = self.current_item.menu_type {
            if let Some(parent) = self.current_item.parent {
                sprintln!(tx, " <0> --> {}", parent.name)?;
            }

            if is_active(ctx) {
                for (i,c) in children.iter().enumerate() {
                    c.menu_item_to_string(i+1, ctx, tx)?;
                }
            }
        }

        Ok(())
    }

    pub fn run<S, E>(&mut self, ctx: &mut Context, serial: &mut S) -> Result<(), nb::Error<E> >
    where
        S: HalRead<Word, Error = E> + HalWrite<Word, Error = E>
    {
        if self.state == MenuState::Init {
            self.display_menu(ctx, serial)?;
            self.state = MenuState::NeedIdx;
        }

        let mut result = Ok(());

        while result.is_ok() {
            let mut changed = false;

            self.get_input(serial)?;

            if let Some(input) = self.buffer.front() {
                let current_idx = if let Some(digit) = input.to_digit(16) {
                    digit as usize
                } else if *input == NEWLINE {
                    let _ = self.buffer.pop_front();
                    self.display_menu(ctx, serial)?;
                    continue;
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
                        changed = true;
                        Ok( () )
                    }
                } else if let MenuItemType::SubMenu(menu, _is_active) = self.current_item.menu_type {
                    self.state = MenuState::NeedIdx;
                    let _ = self.buffer.pop_front();

                    if let Some(child) = menu.get(current_idx - 1) {
                        match child.menu_type {
                            MenuItemType::SubMenu(_, is_active) => {
                                if is_active(ctx) {
                                    self.current_item = child;
                                    changed = true;
                                }
                                else {
                                    sprintln!(serial, "{} is disabled.", child.name)?;
                                }
                                Ok(())
                            }
                            MenuItemType::ReadValue(rcb) => {
                                let mut buffer = String::<U32>::new();
                                rcb(&mut buffer, ctx);
                                sprintln!(serial, "{}: {}", child.name, buffer)
                            }
                            MenuItemType::WriteValue(rcb, _) => {
                                self.state = MenuState::NeedEnter;
                                self.current_item = child;

                                let mut buffer = String::<U32>::new();
                                rcb(&mut buffer, ctx);
                                sprintln!(serial, "{}: {}", child.name, buffer)?;
                                match child.hint {
                                    None => sprintln!(serial, "Enter new value:"),
                                    Some(h) => sprintln!(serial, "Enter new value: [{}]", h),
                                }
                            }
                            MenuItemType::ExecValue(readcb, execcb) => {
                                execcb(ctx);
                                let mut string = String::<U32>::new();
                                readcb(&mut string, ctx);
                                sprintln!(serial, "> {}", string)?;
                                //changed = true;
                                Ok( () )
                            }
                        }
                    } else { Ok( () ) }
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

    static MAIN_MENU: MenuItem<'_, Context> = MenuItem {
        name: "Main",
        hint: None,
        parent: None,
        menu_type: MenuItemType::SubMenu(&[&SUB1, &SUB2], |_| true)
    };

    static BOOL_VAL: MenuItem<'_, Context> = MenuItem {
        name: "Bool",
        hint: Some("boolean"),
        parent: None,
        menu_type: MenuItemType::ReadValue(|buf, ctx| { let _ = write!(buf, "{}", ctx.bool_value); } ),
    };

    static UINT_VAL: MenuItem<'_, Context> = MenuItem {
        name: "Uint",
        hint: None,
        parent: None,
        menu_type: MenuItemType::ReadValue(|buf, ctx| { let _ = write!(buf, "{}", ctx.uint_value); } ),
    };

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
        menu_type: MenuItemType::SubMenu(&[&BOOL_VAL], |_| true)
    };

    static SUB2: MenuItem<'_, Context> = MenuItem {
        name: "Sub Menu 2",
        hint: None,
        parent: Some(&MAIN_MENU),
        menu_type: MenuItemType::SubMenu(&[&UINT_VAL, &UINT_VAL_WRITE], |_| true)
    };

    #[test]
    fn simple() {
        let mut context = Context { bool_value: true, uint_value: 32 };

        let mut runner = Dispatcher::new(&MAIN_MENU).with_refresh();

        // Configure expectations
        let expectations = [
            // Printing Main Menu
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),

            // Printing Sub Menu 1
            SerialTransaction::read(b'1'),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> --> {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {} [{}]\r\n", BOOL_VAL.name, context.bool_value, BOOL_VAL.hint.unwrap() )),
            SerialTransaction::flush(),

            // Printing Main Menu
            SerialTransaction::read(b'0'),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> --> {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {}\r\n", UINT_VAL.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] w=> {}: {}\r\n", UINT_VAL_WRITE.name, context.uint_value)),
            SerialTransaction::flush(),

            // Printing Show UINT_VAL
            SerialTransaction::read(b'1'),
            SerialTransaction::write_many(&format!("{}: {}\r\n", UINT_VAL.name, context.uint_value)),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::read(b'\x08'),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
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
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] --> {}\r\n", SUB1.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] --> {}\r\n", SUB2.name)),
            SerialTransaction::flush(),

            // Printing Sub Menu 2
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many("\r\n"),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!("{}\r\n", SUB2.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" <0> --> {}\r\n", MAIN_MENU.name)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [1] r=> {}: {}\r\n", UINT_VAL.name, context.uint_value)),
            SerialTransaction::flush(),
            SerialTransaction::write_many(&format!(" [2] w=> {}: {}\r\n", UINT_VAL_WRITE.name, context.uint_value)),
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
            SerialTransaction::flush(),
            SerialTransaction::flush(),
            SerialTransaction::read(BACKSPACE as u8), // backspace
            SerialTransaction::write_many(format!("{bs} {bs}", bs=BACKSPACE) ),
            SerialTransaction::flush(),
            SerialTransaction::read(b'2'),
            SerialTransaction::write_many("2"),
            SerialTransaction::flush(),
            SerialTransaction::flush(),
            SerialTransaction::read(b'5'),
            SerialTransaction::write_many("5"),
            SerialTransaction::flush(),
            SerialTransaction::flush(),
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
