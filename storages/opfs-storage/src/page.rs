use gluesql_core::error::{Error, Result};

// Page layout:
// `[magic 4B][version 1B][kind 1B][reserved 2B][next 4B][payload len 4B]`
// `[auxiliary 8B][checksum 8B][payload]`.
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_PAYLOAD_SIZE: usize = PAGE_SIZE - PAGE_HEADER_LEN;

const PAGE_MAGIC: [u8; 4] = *b"GLPG";
const PAGE_VERSION: u8 = 1;
const PAGE_HEADER_LEN: usize = 32;
const NONE_PAGE_ID: u32 = u32::MAX;
const CHECKSUM_START: usize = 24;
const CHECKSUM_END: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(pub u32);

impl PageId {
    #[must_use]
    pub fn offset(self) -> u64 {
        u64::from(self.0) * PAGE_SIZE as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Meta,
    Free,
    RecordHead,
    RecordBody,
}

impl PageKind {
    fn encode(self) -> u8 {
        match self {
            Self::Meta => 1,
            Self::Free => 2,
            Self::RecordHead => 3,
            Self::RecordBody => 4,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Meta),
            2 => Ok(Self::Free),
            3 => Ok(Self::RecordHead),
            4 => Ok(Self::RecordBody),
            _ => Err(storage_error(format!("unknown page kind {value}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    kind: PageKind,
    next: Option<PageId>,
    auxiliary: u64,
    payload: Vec<u8>,
}

impl Page {
    pub fn new(
        kind: PageKind,
        next: Option<PageId>,
        auxiliary: u64,
        payload: Vec<u8>,
    ) -> Result<Self> {
        if payload.len() > PAGE_PAYLOAD_SIZE {
            return Err(storage_error(format!(
                "page payload is too large: {} > {PAGE_PAYLOAD_SIZE}",
                payload.len()
            )));
        }

        Ok(Self {
            kind,
            next,
            auxiliary,
            payload,
        })
    }

    pub fn free(next: Option<PageId>) -> Self {
        Self {
            kind: PageKind::Free,
            next,
            auxiliary: 0,
            payload: Vec::new(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> PageKind {
        self.kind
    }

    #[must_use]
    pub fn next(&self) -> Option<PageId> {
        self.next
    }

    #[must_use]
    pub fn auxiliary(&self) -> u64 {
        self.auxiliary
    }

    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn encode(&self) -> Result<[u8; PAGE_SIZE]> {
        let payload_len = u32::try_from(self.payload.len())
            .map_err(|_| storage_error("page payload length does not fit in u32"))?;
        let mut bytes = [0; PAGE_SIZE];
        bytes[..4].copy_from_slice(&PAGE_MAGIC);
        bytes[4] = PAGE_VERSION;
        bytes[5] = self.kind.encode();
        bytes[8..12].copy_from_slice(&encode_page_id(self.next).to_le_bytes());
        bytes[12..16].copy_from_slice(&payload_len.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.auxiliary.to_le_bytes());
        bytes[PAGE_HEADER_LEN..PAGE_HEADER_LEN + self.payload.len()].copy_from_slice(&self.payload);

        let checksum = page_checksum(&bytes);
        bytes[CHECKSUM_START..CHECKSUM_END].copy_from_slice(&checksum.to_le_bytes());

        Ok(bytes)
    }

    pub fn decode(bytes: &[u8; PAGE_SIZE]) -> Result<Self> {
        if bytes[..4] != PAGE_MAGIC {
            return Err(storage_error("invalid page magic"));
        }

        if bytes[4] != PAGE_VERSION {
            return Err(storage_error(format!(
                "unsupported page version {}, expected {PAGE_VERSION}",
                bytes[4]
            )));
        }

        let expected = u64::from_le_bytes(bytes[CHECKSUM_START..CHECKSUM_END].try_into().unwrap());
        let actual = page_checksum(bytes);

        if actual != expected {
            return Err(storage_error("page checksum mismatch"));
        }

        let kind = PageKind::decode(bytes[5])?;
        let next = decode_page_id(u32::from_le_bytes(bytes[8..12].try_into().unwrap()));
        let payload_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;

        if payload_len > PAGE_PAYLOAD_SIZE {
            return Err(storage_error("page payload length is out of range"));
        }

        let auxiliary = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let payload = bytes[PAGE_HEADER_LEN..PAGE_HEADER_LEN + payload_len].to_vec();

        Ok(Self {
            kind,
            next,
            auxiliary,
            payload,
        })
    }
}

#[must_use]
pub fn checksum(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

fn page_checksum(bytes: &[u8; PAGE_SIZE]) -> u64 {
    let mut hash_bytes = Vec::with_capacity(PAGE_SIZE - (CHECKSUM_END - CHECKSUM_START));
    hash_bytes.extend_from_slice(&bytes[..CHECKSUM_START]);
    hash_bytes.extend_from_slice(&bytes[CHECKSUM_END..]);

    checksum(&hash_bytes)
}

fn encode_page_id(page_id: Option<PageId>) -> u32 {
    page_id.map_or(NONE_PAGE_ID, |page_id| page_id.0)
}

fn decode_page_id(page_id: u32) -> Option<PageId> {
    (page_id != NONE_PAGE_ID).then_some(PageId(page_id))
}

fn storage_error(message: impl Into<String>) -> Error {
    Error::StorageMsg(format!("opfs-storage: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_roundtrip() {
        let page = Page::new(
            PageKind::RecordHead,
            Some(PageId(9)),
            123,
            b"hello".to_vec(),
        )
        .unwrap();

        assert_eq!(Page::decode(&page.encode().unwrap()).unwrap(), page);
    }

    #[test]
    fn rejects_corrupt_page() {
        let page = Page::free(None);
        let mut bytes = page.encode().unwrap();
        bytes[PAGE_HEADER_LEN] ^= 0xff;

        assert!(Page::decode(&bytes).is_err());
    }
}
