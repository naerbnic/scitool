
#[doc(hidden)]
pub mod support;

pub use datalit_macros::datalit;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8_literals() {
        let bytes = datalit!(1u8, 2u8, 3u8);
        assert_eq!(bytes, vec![1u8, 2u8, 3u8]);
    }

    #[test]
    fn test_endian_literals() {
        let bytes = datalit!(1u16_le, 2u16_be);
        assert_eq!(bytes, vec![1u8, 0u8, 0u8, 2u8]);
    }
}
