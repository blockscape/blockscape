use std::cmp;

static BASE16_DIGITS: [char; 16] = 
        ['0', '1', '2', '3', '4', '5', '6', '7',
         '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'];

/// A quick and dirty print for use in data storage and transfer.
pub fn from_bin(bin: &[u8]) -> String {
    let mut base16 = String::with_capacity(bin.len() * 2);
    for byte in bin {
        let upper = byte >> 4;
        let lower = byte & 0x0F;
        
        base16.push(BASE16_DIGITS[upper as usize]);
        base16.push(BASE16_DIGITS[lower as usize]);
    } base16
}

/// Convert a hex string to a binary value
/// # Errors
/// * If the string is empty.
/// * If any character is invalid.
pub fn to_bin(base16: &str) -> Result<Vec<u8>, &'static str> {
    let mut i = base16.len() as i64; // i is one beyond the end
    let mut s: &str = base16;

    // Remove front 0x if there is one
    if (i >= 3) && (&s[0..2] == "0x") {
        s = &s[2..];
        i -= 2;
    }

    // make sure it is not empty
    if i <= 0 {
        return Err("Value is empty.");
    }

    let mut result: Vec<u8> = Vec::with_capacity((i/2) as usize);

    // Convert the individual segments to u8 s and add to the data.
    while i > 0 {
        // grab a u8's width of hex digits
        let str_range = (cmp::max(i - 2, 0) as usize)..(i as usize);
        let str_segment = &s[str_range];

        // convert the u8 hex digits to a u8
        let str_value = u8::from_str_radix(str_segment, 16);
        match str_value {
            Ok(value) => result.push(value),
            Err(_) => return Err("Invalid character.")
        }

        // increment our position
        i -= 2;
    }

    result.reverse();
    Ok(result)
}



#[cfg(test)]
mod test { // see also U160 and U256 testing
    static A1: &'static str = "00ff225e06";
    static A2: &'static str = "572b71";
    static A3: &'static str = "7b382020555a";
    static A4: &'static str = "*&*&^%$%^&*";

    static B1: [u8; 5] = [0, 255, 34, 94, 6];
    static B2: [u8; 3] = [87, 43, 113];
    static B3: [u8; 6] = [123, 56, 32, 32, 85, 90];

    #[test]
    fn from_bin() {
        assert_eq!(super::from_bin(&B1), A1);
        assert_eq!(super::from_bin(&B2), A2);
        assert_eq!(super::from_bin(&B3), A3);
    }

    #[test]
    fn to_bin() {
        assert_eq!(super::to_bin(A1).unwrap(), B1);
        assert_eq!(super::to_bin(A2).unwrap(), B2);
        assert_eq!(super::to_bin(A3).unwrap(), B3);
        assert!(super::to_bin(A4).is_err());
    }
}