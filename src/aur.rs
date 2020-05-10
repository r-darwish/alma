use crate::error::ErrorKind;
use std::str::FromStr;

pub struct AurHelper {
    pub name: String,
    pub install_command: Vec<String>,
}

impl FromStr for AurHelper {
    type Err = ErrorKind;
    // Remove make dependencies after install? [y/N]
    // :: Proceed with installation? [Y/n]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yay" => Ok(AurHelper {
                name: String::from("yay"),
                install_command: vec![
                    String::from("yay"),
                    String::from("-S"),
                    String::from("--nocleanmenu"),
                    String::from("--nodiffmenu"),
                    String::from("--noeditmenu"),
                    String::from("--noupgrademenu"),
                    String::from("--useask"),
                    String::from("--removemake"),
                    String::from("--norebuild"),
                    String::from("--noconfirm"),
                    String::from("--answeredit"),
                    String::from("None"),
                    String::from("--answerclean"),
                    String::from("None"),
                    String::from("--mflags"),
                    String::from("--noconfirm"),
                ],
            }),
            _ => Err(ErrorKind::AurHelper {}),
        }
    }
}
