use std::io;

macro_rules! prim_write_le_fn {
    ($name:ident, $ty:ty) => {
        fn $name(&mut self, value: $ty) -> io::Result<()> {
            self.write_all(&value.to_le_bytes())
        }
    };
}

pub trait DataWriterExt: io::Write {
    prim_write_le_fn!(write_u8, u8);
    prim_write_le_fn!(write_u16_le, u16);
    prim_write_le_fn!(write_u32_le, u32);
    prim_write_le_fn!(write_u64_le, u64);
    prim_write_le_fn!(write_i8, i8);
    prim_write_le_fn!(write_i16_le, i16);
    prim_write_le_fn!(write_i32_le, i32);
    prim_write_le_fn!(write_i64_le, i64);
    prim_write_le_fn!(write_f32_le, f32);
    prim_write_le_fn!(write_f64_le, f64);
}

impl<W: io::Write> DataWriterExt for W {}

macro_rules! prim_read_le_fn {
    ($name:ident, $ty:ty) => {
        fn $name(&mut self) -> io::Result<$ty> {
            let mut bytes = [0; std::mem::size_of::<$ty>()];
            self.read_exact(&mut bytes)?;
            Ok(<$ty>::from_le_bytes(bytes))
        }
    };
}

pub trait DataReaderExt: io::Read {
    prim_read_le_fn!(read_u8, u8);
    prim_read_le_fn!(read_u16_le, u16);
    prim_read_le_fn!(read_u32_le, u32);
    prim_read_le_fn!(read_u64_le, u64);
    prim_read_le_fn!(read_i8, i8);
    prim_read_le_fn!(read_i16_le, i16);
    prim_read_le_fn!(read_i32_le, i32);
    prim_read_le_fn!(read_i64_le, i64);
    prim_read_le_fn!(read_f32_le, f32);
    prim_read_le_fn!(read_f64_le, f64);
}

impl<R: io::Read> DataReaderExt for R {}
