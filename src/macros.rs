//! Macro Collection


/// Try to print over serial
#[macro_export]
macro_rules! sprint {
    ($stdout:expr, $($arg:tt)*) => ({
        let mut output: String<U100> = String::new();
        if core::fmt::write(&mut output, format_args!($($arg)*)).is_ok() {

            #[cfg(test)]
            {
                print!("{}", output);
            }

            for b in output.as_bytes() {
                $stdout.write(*b)?;
                if *b == b'\n' {
                    $stdout.flush()?;
                }
            }
            Ok( () )
        }
        else {
            Err(nb::Error::WouldBlock)
        }
    })
}

/// Try to print a line over serial
#[macro_export]
macro_rules! sprintln {
    ($stdout:expr)                         => { sprint!($stdout, "\r\n") };
    ($stdout:expr, $fmt:expr)              => { sprint!($stdout, concat!($fmt, "\r\n") ) };
    ($stdout:expr, $fmt:expr, $($arg:tt)*) => { sprint!($stdout, concat!($fmt, "\r\n"), $($arg)*) };
}
/*
#[macro_export]
macro_rules! mitem {
    (Main,
     $ident_name:ident = $name:expr,
     [$( $child_name:expr => $children:tt, )*]
    ) => {
        $(
            mitem!($childname:expr; $children:tt)
        )+

        static $ident_name: MenuItem<'_, Context> = MenuItem {
            name: $name,
            hint: None,
            parent: None,
            menu_type: MenuItemType::SubMenu(&[$( &$childname )+ ]),
        };
    };
    ($ident_name:ident => $parent:expr
     Read (&$parent:ident),
      = $name:expr,
     r=> $rfunc:expr
    ) => {
        static $ident_name: MenuItem<'_, Context> = MenuItem {
            name: $name,
            hint: None,
            parent: Some(&$parent),
            menu_type: MenuItemType::ReadValue($rfunc),
        };
    };
}
*/
