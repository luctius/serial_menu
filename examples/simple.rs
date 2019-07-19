//! Simple Serial Menu Example
//!
//! We use ncurses to get around stdin buffer and be able to detect backspaces etc.

use serial_menu::*;

use pancurses::{initscr, endwin};

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

struct SerialMock {
    window: pancurses::Window,
}
impl<'a> embedded_hal::serial::Read<u8> for SerialMock {
    type Error = std::io::Error;

    fn read(&mut self) -> Result<u8, nb::Error<Self::Error> > {
        match self.window.getch().unwrap() {
            pancurses::Input::KeyEnter => Ok(b'\n'),
            pancurses::Input::KeyBackspace => Ok(b'\x08'),
            pancurses::Input::Character(c) => Ok(c as u8),
            _ => Ok('a' as u8),
        }
    }
}
impl<'a> embedded_hal::serial::Write<u8> for SerialMock {
    type Error = std::io::Error;

    fn write(&mut self, ch: u8) -> Result<(), nb::Error<Self::Error> > {
        /* No Idea WHY I need this, but printing ch gives weird results */
        match ch {
            b'\r' => {},
            b'\n' => { self.window.addch('\n'); },
            b' ' => { self.window.addch(' '); },
            b':' => { self.window.addch(':'); },
            b'-' => { self.window.addch('-'); },
            b'_' => { self.window.addch('_'); },
            b'[' => { self.window.addch('['); },
            b']' => { self.window.addch(']'); },
            b'{' => { self.window.addch('{'); },
            b'}' => { self.window.addch('}'); },
            b'<' => { self.window.addch('<'); },
            b'>' => { self.window.addch('>'); },
            b'a'  => { self.window.addch('a'); },
            b'b'  => { self.window.addch('b'); },
            b'c'  => { self.window.addch('c'); },
            b'd'  => { self.window.addch('d'); },
            b'e'  => { self.window.addch('e'); },
            b'f'  => { self.window.addch('f'); },
            b'g'  => { self.window.addch('g'); },
            b'h'  => { self.window.addch('h'); },
            b'i'  => { self.window.addch('i'); },
            b'j'  => { self.window.addch('j'); },
            b'k'  => { self.window.addch('k'); },
            b'l'  => { self.window.addch('l'); },
            b'm'  => { self.window.addch('m'); },
            b'n'  => { self.window.addch('n'); },
            b'o'  => { self.window.addch('o'); },
            b'p'  => { self.window.addch('p'); },
            b'q'  => { self.window.addch('q'); },
            b'r'  => { self.window.addch('r'); },
            b's'  => { self.window.addch('s'); },
            b't'  => { self.window.addch('t'); },
            b'u'  => { self.window.addch('u'); },
            b'v'  => { self.window.addch('v'); },
            b'w'  => { self.window.addch('w'); },
            b'x'  => { self.window.addch('x'); },
            b'y'  => { self.window.addch('y'); },
            b'z'  => { self.window.addch('z'); },
            b'A'  => { self.window.addch('A'); },
            b'B'  => { self.window.addch('B'); },
            b'C'  => { self.window.addch('C'); },
            b'D'  => { self.window.addch('D'); },
            b'E'  => { self.window.addch('E'); },
            b'F'  => { self.window.addch('F'); },
            b'G'  => { self.window.addch('G'); },
            b'H'  => { self.window.addch('H'); },
            b'I'  => { self.window.addch('I'); },
            b'J'  => { self.window.addch('J'); },
            b'K'  => { self.window.addch('K'); },
            b'L'  => { self.window.addch('L'); },
            b'M'  => { self.window.addch('M'); },
            b'N'  => { self.window.addch('N'); },
            b'O'  => { self.window.addch('O'); },
            b'P'  => { self.window.addch('P'); },
            b'Q'  => { self.window.addch('Q'); },
            b'R'  => { self.window.addch('R'); },
            b'S'  => { self.window.addch('S'); },
            b'T'  => { self.window.addch('T'); },
            b'U'  => { self.window.addch('U'); },
            b'V'  => { self.window.addch('V'); },
            b'W'  => { self.window.addch('W'); },
            b'X'  => { self.window.addch('X'); },
            b'Y'  => { self.window.addch('Y'); },
            b'Z'  => { self.window.addch('Z'); },
            b'0'  => { self.window.addch('0'); },
            b'1'  => { self.window.addch('1'); },
            b'2'  => { self.window.addch('2'); },
            b'3'  => { self.window.addch('3'); },
            b'4'  => { self.window.addch('4'); },
            b'5'  => { self.window.addch('5'); },
            b'6'  => { self.window.addch('6'); },
            b'7'  => { self.window.addch('7'); },
            b'8'  => { self.window.addch('8'); },
            b'9'  => { self.window.addch('9'); },
            _ => {},
        }
        Ok( () )
    }
    fn flush(&mut self) -> Result<(), nb::Error<Self::Error> > {
        self.window.refresh();
        Ok( () )
    }
}

fn main() {
    let mut context = Context { bool_value: true, uint_value: 32 };

    let mut runner = Dispatcher::new(&MAIN_MENU).with_refresh();

    /* Setup ncurses. */
    let window = initscr();
    window.refresh();
    window.keypad(true);
    pancurses::nonl();
    pancurses::noecho();

    let mut serial = SerialMock { window };

        match runner.run(&mut context, &mut serial) {
            Ok(_) => (),
            Err(Error::InvalidInput) => {},
            Err(Error::Hardware(e)) => {
                match e {
                    nb::Error::WouldBlock => {},
                    nb::Error::Other(_) => { panic!("Hardware error: {:?}", e); },
                }
            },
        }
}
