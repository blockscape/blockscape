use range::Range;

static BASE16_DIGITS: [char; 16] = 
        ['0', '1', '2', '3', '4', '5', '6', '7',
         '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'];

pub fn bin_to_base16(bin: &[u8]) -> String {
    let mut base16 = String::with_capacity(bin.len() * 2);
    for byte in bin {
        let upper = byte >> 4;
        let lower = byte & 0x0F;
        
        base16.push(BASE16_DIGITS[upper as usize]);
        base16.push(BASE16_DIGITS[lower as usize]);
    } base16
}

pub fn base16_to_bin(base16: &str) -> Result<Vec<u8>, &'static str> {
    if base16.len() % 2 == 1 { return Err("Invalid encoding, string must be of an even length.") }
    let mut bin: Vec<u8> = Vec::with_capacity(base16.len() / 2);
    for i in Range(0, base16.len(), 2) {
        let byte = u8::from_str_radix(&base16[i..i+2], 16)
            .map_err(|_| "Invalid encoding, unknown character detected.")?;
        bin.push(byte);
    } Ok(bin)
}



#[cfg(test)]
mod test {
    static A1: &'static str = "00ff225e06";
    static A2: &'static str = "572b71";
    static A3: &'static str = "7b382020555a";
    static A4: &'static str = "*&*&^%$%^&*";

    static B1: [u8; 5] = [0, 255, 34, 94, 6];
    static B2: [u8; 3] = [87, 43, 113];
    static B3: [u8; 6] = [123, 56, 32, 32, 85, 90];

    #[test]
    fn bin_to_base16() {
        assert_eq!(super::bin_to_base16(&B1), A1);
        assert_eq!(super::bin_to_base16(&B2), A2);
        assert_eq!(super::bin_to_base16(&B3), A3);
    }

    #[test]
    fn base16_to_bin() {
        assert_eq!(super::base16_to_bin(A1).unwrap(), B1);
        assert_eq!(super::base16_to_bin(A2).unwrap(), B2);
        assert_eq!(super::base16_to_bin(A3).unwrap(), B3);
        assert!(super::base16_to_bin(A4).is_err());
    }
}