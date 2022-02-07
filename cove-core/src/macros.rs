macro_rules! packets {
    ( $( $name:ident($cmd:ident, $rpl:ident), )* ) => {
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(tag = "name", content = "data")]
        pub enum Cmd {
            $( $name($cmd), )*
        }

        $(
            impl std::convert::TryFrom<Cmd> for $cmd {
                type Error = ();
                fn try_from(cmd: Cmd) -> Result<Self, Self::Error> {
                    match cmd {
                        Cmd::$name(val) => Ok(val),
                        _ => Err(()),
                    }
                }
            }
        )*

        #[derive(Debug, Deserialize, Serialize)]
        #[serde(tag = "name", content = "data")]
        pub enum Rpl {
            $( $name($rpl), )*
        }

        $(
            impl std::convert::TryFrom<Rpl> for $rpl {
                type Error = ();
                fn try_from(rpl: Rpl) -> Result<Self, Self::Error> {
                    match rpl {
                        Rpl::$name(val) => Ok(val),
                        _ => Err(()),
                    }
                }
            }
        )*
    };
}

// Make macro importable from elsewhere
// See https://stackoverflow.com/a/31749071
pub(crate) use packets;
