use std::fmt::Write;

pub struct Initcpio {
    encrypted: bool,
}

impl Initcpio {
    pub fn new(encrypted: bool) -> Self {
        Self { encrypted }
    }

    pub fn to_config(&self) -> anyhow::Result<String> {
        let mut output = String::from(
            "MODULES=()
BINARIES=()
FILES=()
HOOKS=(base udev keyboard consolefont block ",
        );

        if self.encrypted {
            output.write_str("encrypt ")?;
        }

        output.write_str("filesystems keyboard fsck)\n")?;

        Ok(output)
    }
}
