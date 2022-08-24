use crate::{MessageType, Property, ProtocolError as Error};
use bit_field::BitField;
use heapless::Vec;
use varint_rs::VarintReader;

pub(crate) struct PacketParser<'a> {
    pub buffer: &'a [u8],
    index: core::cell::RefCell<usize>,
}

impl<'a> PacketParser<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            index: core::cell::RefCell::new(0),
        }
    }

    pub fn payload(&self) -> Result<&[u8], Error> {
        Ok(&self.buffer[*self.index.borrow()..self.buffer.len()])
    }

    pub fn read(&self, dest: &mut [u8]) -> Result<(), Error> {
        let mut index = self.index.borrow_mut();

        if *index + dest.len() > self.buffer.len() {
            return Err(Error::DataSize);
        }

        dest.copy_from_slice(&self.buffer[*index..][..dest.len()]);
        *index += dest.len();

        Ok(())
    }

    fn read_borrowed(&self, count: usize) -> Result<&[u8], Error> {
        let mut index = self.index.borrow_mut();

        if *index + count > self.buffer.len() {
            return Err(Error::DataSize);
        }

        let borrowed_data = &self.buffer[*index..][..count];
        *index += count;

        Ok(borrowed_data)
    }

    pub fn len(&self) -> Result<usize, Error> {
        Ok(self.buffer.len() - *self.index.borrow())
    }

    pub fn read_variable_length_integer(&self) -> Result<u32, Error> {
        let mut reader = IntReader { parser: self };

        reader.read_u32_varint()
    }

    pub fn read_fixed_header(&self) -> Result<(MessageType, u8, usize), Error> {
        let header = self.read_u8()?;
        let packet_length = self.read_variable_length_integer()?;

        Ok((
            MessageType::from(header.get_bits(4..=7)),
            header.get_bits(0..=3),
            packet_length as usize,
        ))
    }

    pub fn read_utf8_string<'b, 'me: 'b>(&'me self) -> Result<&'b str, Error> {
        let string_length = self.read_u16()? as usize;

        if self.buffer.len() < string_length {
            return Err(Error::DataSize);
        }

        core::str::from_utf8(self.read_borrowed(string_length)?).map_err(|_| Error::MalformedPacket)
    }

    pub fn read_binary_data(&self) -> Result<&[u8], Error> {
        let string_length = self.read_u16()? as usize;
        self.read_borrowed(string_length)
    }

    pub fn read_u32(&self) -> Result<u32, Error> {
        let mut buffer: [u8; 4] = [0; 4];
        self.read(&mut buffer)?;
        Ok(u32::from_be_bytes(buffer))
    }

    pub fn read_u16(&self) -> Result<u16, Error> {
        let mut buffer: [u8; 2] = [0; 2];
        self.read(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }

    pub fn read_u8(&self) -> Result<u8, Error> {
        let mut byte: [u8; 1] = [0];
        self.read(&mut byte)?;

        Ok(byte[0])
    }

    pub fn read_properties<'b, 'me: 'b>(&'me self) -> Result<Vec<Property<'b>, 8>, Error> {
        let mut properties: Vec<Property, 8> = Vec::new();

        let properties_size = self.read_variable_length_integer()? as usize;

        let remainder = self.len()? - properties_size;

        while self.len()? > remainder {
            properties
                .push(Property::parse(self)?)
                .map_err(|_| Error::MalformedPacket)?;
        }

        if self.len()? != remainder {
            return Err(Error::MalformedPacket);
        }

        Ok(properties)
    }
}

pub struct IntReader<'a> {
    parser: &'a PacketParser<'a>,
}

impl<'a> varint_rs::VarintReader for IntReader<'a> {
    type Error = Error;
    fn read(&mut self) -> Result<u8, Error> {
        self.parser.read_u8()
    }
}
