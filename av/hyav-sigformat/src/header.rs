//! HyAV Signature Database Header definition and serialization.
//! Magic: HYAVSIG\0 (8 bytes) followed by 56 bytes of metadata = 64 bytes total.

pub const SIG_MAGIC: [u8; 8] = *b"HYAVSIG\0";
pub const HEADER_SIZE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DbType {
    Main = 0x01,
    Daily = 0x02,
    Custom = 0x03,
    Emergency = 0x04,
}

impl DbType {
    pub fn from_u16(val: u16) -> Option<Self> {
        match val {
            0x01 => Some(DbType::Main),
            0x02 => Some(DbType::Daily),
            0x03 => Some(DbType::Custom),
            0x04 => Some(DbType::Emergency),
            _ => None,
        }
    }
}

macro_rules! bitflags_lite {
    (pub struct $name:ident: $type:ty { $(const $const_name:ident = $val:expr;)* }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name(pub $type);
        impl $name {
            $(pub const $const_name: Self = Self($val);)*
            pub fn contains(&self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }
        }
    };
}
pub(crate) use bitflags_lite;

bitflags_lite! {
    pub struct HeaderFlags: u16 {
        const NONE = 0x0000;
        const COMPRESSED = 0x0001;
        const STRICT_MODE = 0x0002;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub magic: [u8; 8],
    pub version_major: u16,
    pub version_minor: u16,
    pub db_type: DbType,
    pub flags: HeaderFlags,
    pub timestamp: u64,
    pub record_count: u32,
    pub bloom_offset: u32,
    pub bloom_size: u32,
    pub records_offset: u32,
    pub records_size: u32,
    pub trailer_offset: u32,
    pub reserved: [u8; 20],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    BufferTooSmall,
    InvalidMagic,
    UnsupportedVersion,
    InvalidDbType,
}

impl Header {
    pub fn new(
        db_type: DbType,
        record_count: u32,
        bloom_offset: u32,
        bloom_size: u32,
        records_offset: u32,
        records_size: u32,
        trailer_offset: u32,
    ) -> Self {
        Self {
            magic: SIG_MAGIC,
            version_major: 1,
            version_minor: 0,
            db_type,
            flags: HeaderFlags::NONE,
            timestamp: 0,
            record_count,
            bloom_offset,
            bloom_size,
            records_offset,
            records_size,
            trailer_offset,
            reserved: [0u8; 20],
        }
    }

    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..8].copy_from_slice(&self.magic);
        buf[8..10].copy_from_slice(&self.version_major.to_le_bytes());
        buf[10..12].copy_from_slice(&self.version_minor.to_le_bytes());
        buf[12..14].copy_from_slice(&(self.db_type as u16).to_le_bytes());
        buf[14..16].copy_from_slice(&self.flags.0.to_le_bytes());
        buf[16..24].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[24..28].copy_from_slice(&self.record_count.to_le_bytes());
        buf[28..32].copy_from_slice(&self.bloom_offset.to_le_bytes());
        buf[32..36].copy_from_slice(&self.bloom_size.to_le_bytes());
        buf[36..40].copy_from_slice(&self.records_offset.to_le_bytes());
        buf[40..44].copy_from_slice(&self.records_size.to_le_bytes());
        buf[44..48].copy_from_slice(&self.trailer_offset.to_le_bytes());
        buf[48..64].copy_from_slice(&self.reserved);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HeaderError> {
        if bytes.len() < HEADER_SIZE {
            return Err(HeaderError::BufferTooSmall);
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);
        if magic != SIG_MAGIC {
            return Err(HeaderError::InvalidMagic);
        }

        let version_major = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        let version_minor = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
        if version_major != 1 {
            return Err(HeaderError::UnsupportedVersion);
        }

        let db_type_raw = u16::from_le_bytes(bytes[12..14].try_into().unwrap());
        let db_type = DbType::from_u16(db_type_raw).ok_or(HeaderError::InvalidDbType)?;

        let flags = HeaderFlags(u16::from_le_bytes(bytes[14..16].try_into().unwrap()));
        let timestamp = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let record_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let bloom_offset = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        let bloom_size = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        let records_offset = u32::from_le_bytes(bytes[36..40].try_into().unwrap());
        let records_size = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        let trailer_offset = u32::from_le_bytes(bytes[44..48].try_into().unwrap());

        let mut reserved = [0u8; 20];
        reserved.copy_from_slice(&bytes[48..64]);

        Ok(Self {
            magic,
            version_major,
            version_minor,
            db_type,
            flags,
            timestamp,
            record_count,
            bloom_offset,
            bloom_size,
            records_offset,
            records_size,
            trailer_offset,
            reserved,
        })
    }
}
