use anyhow::anyhow;
use std::str::FromStr;

pub struct AurHelper {
    pub name: String,
    pub package_name: String,
    pub install_command: Vec<String>,
}

impl FromStr for AurHelper {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "paru" => Ok(Self {
                name: String::from("paru"),
                package_name: String::from("paru-bin"),
                install_command: vec![
                    String::from("paru"),
                    String::from("-S"),
                    String::from("--skipreview"),
                    String::from("--noupgrademenu"),
                    String::from("--useask"),
                    String::from("--removemake"),
                    String::from("--norebuild"),
                    String::from("--nocleanafter"),
                    String::from("--noredownload"),
                    String::from("--mflags"),
                    String::from(""),
                    String::from("--noconfirm"),
                    String::from("--batchinstall"),
                ],
            }),
            "yay" => Ok(Self {
                name: String::from("yay"),
                package_name: String::from("yay-bin"),
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
                    String::from("--answeredit"),
                    String::from("None"),
                    String::from("--answerclean"),
                    String::from("None"),
                    String::from("--mflags"),
                    String::from("--noconfirm"),
                ],
            }),
            _ => Err(anyhow!("Error parsing AUR helper string: {}", s)),
        }
    }
}
