use std::fs::File;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowsFileInformation {
    pub(crate) file_attributes: u64,
    pub(crate) creation_time: Option<u64>,
    pub(crate) last_write_time: Option<u64>,
    pub(crate) volume_serial_number: u32,
    pub(crate) file_size: u64,
    pub(crate) file_index: u64,
    pub(crate) number_of_links: u64,
}

pub(crate) fn file_information(file: &File) -> io::Result<WindowsFileInformation> {
    let information = winapi_util::file::information(file)?;
    Ok(WindowsFileInformation {
        file_attributes: information.file_attributes(),
        creation_time: information.creation_time(),
        last_write_time: information.last_write_time(),
        volume_serial_number: u32::try_from(information.volume_serial_number())
            .expect("Windows volume serial number is represented by a u32"),
        file_size: information.file_size(),
        file_index: information.file_index(),
        number_of_links: information.number_of_links(),
    })
}
