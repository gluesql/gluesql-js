use {
    crate::{
        file::RandomAccessFile,
        page::{PAGE_SIZE, Page, PageId, checksum},
    },
    gluesql_core::error::{Error, Result},
    std::collections::BTreeMap,
};

// A transaction stores complete after-images for every dirty page. Commit flushes the WAL first,
// applies data pages and metadata to the database, flushes the database, and only then clears the
// WAL. Recovery can therefore replay a committed transaction idempotently after an interrupted
// database write; a WAL without a complete commit footer is discarded.
const WAL_MAGIC: [u8; 4] = *b"GLWL";
const WAL_VERSION: u8 = 1;
const COMMIT_MAGIC: [u8; 4] = *b"CMIT";
const HEADER_LEN: usize = 24;
const FRAME_LEN: usize = 4 + PAGE_SIZE;
const FOOTER_LEN: usize = 12;

struct WalTransaction {
    pages: BTreeMap<PageId, Page>,
}

pub fn recover<D, W>(database: &D, wal: &W) -> Result<()>
where
    D: RandomAccessFile,
    W: RandomAccessFile,
{
    let bytes = wal.read_all()?;

    if bytes.is_empty() {
        return Ok(());
    }

    let Some(transaction) = decode_transaction(&bytes)? else {
        clear(wal)?;

        return Ok(());
    };

    apply(database, &transaction.pages)?;
    clear(wal)
}

pub fn commit<D, W>(database: &D, wal: &W, tx_id: u64, pages: &BTreeMap<PageId, Page>) -> Result<()>
where
    D: RandomAccessFile,
    W: RandomAccessFile,
{
    let bytes = encode_transaction(tx_id, pages)?;
    wal.truncate(0)?;
    wal.write_at(0, &bytes)?;
    wal.truncate(bytes.len() as u64)?;
    wal.flush()?;

    apply(database, pages)?;
    clear(wal)
}

fn apply<D>(database: &D, pages: &BTreeMap<PageId, Page>) -> Result<()>
where
    D: RandomAccessFile,
{
    for (page_id, page) in pages.iter().filter(|(page_id, _)| page_id.0 != 0) {
        database.write_at(page_id.offset(), &page.encode()?)?;
    }

    if let Some(meta) = pages.get(&PageId(0)) {
        database.write_at(0, &meta.encode()?)?;
    }

    database.flush()
}

fn clear<W>(wal: &W) -> Result<()>
where
    W: RandomAccessFile,
{
    wal.truncate(0)?;
    wal.flush()
}

fn encode_transaction(tx_id: u64, pages: &BTreeMap<PageId, Page>) -> Result<Vec<u8>> {
    let frame_count = u32::try_from(pages.len())
        .map_err(|_| storage_error("WAL contains too many page frames"))?;
    let frames_len = pages
        .len()
        .checked_mul(FRAME_LEN)
        .ok_or_else(|| storage_error("WAL length overflow"))?;
    let capacity = HEADER_LEN
        .checked_add(frames_len)
        .and_then(|len| len.checked_add(FOOTER_LEN))
        .ok_or_else(|| storage_error("WAL length overflow"))?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(&WAL_MAGIC);
    bytes.push(WAL_VERSION);
    bytes.extend_from_slice(&[0; 3]);
    bytes.extend_from_slice(&tx_id.to_le_bytes());
    bytes.extend_from_slice(&frame_count.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);

    for (page_id, page) in pages {
        bytes.extend_from_slice(&page_id.0.to_le_bytes());
        bytes.extend_from_slice(&page.encode()?);
    }

    let transaction_checksum = checksum(&bytes);
    bytes.extend_from_slice(&COMMIT_MAGIC);
    bytes.extend_from_slice(&transaction_checksum.to_le_bytes());

    Ok(bytes)
}

fn decode_transaction(bytes: &[u8]) -> Result<Option<WalTransaction>> {
    if bytes.len() < HEADER_LEN {
        return Ok(None);
    }

    if bytes[..4] != WAL_MAGIC {
        return Err(storage_error("invalid WAL magic"));
    }

    if bytes[4] != WAL_VERSION {
        return Err(storage_error(format!(
            "unsupported WAL version {}, expected {WAL_VERSION}",
            bytes[4]
        )));
    }

    let tx_id = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let frame_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let frames_len = frame_count
        .checked_mul(FRAME_LEN)
        .ok_or_else(|| storage_error("WAL length overflow"))?;
    let expected_len = HEADER_LEN
        .checked_add(frames_len)
        .and_then(|len| len.checked_add(FOOTER_LEN))
        .ok_or_else(|| storage_error("WAL length overflow"))?;

    if bytes.len() < expected_len {
        return Ok(None);
    }

    if bytes.len() != expected_len {
        return Err(storage_error("WAL has an unexpected trailing region"));
    }

    let footer = HEADER_LEN + frames_len;

    if bytes[footer..footer + 4] != COMMIT_MAGIC {
        return Ok(None);
    }

    let expected_checksum =
        u64::from_le_bytes(bytes[footer + 4..footer + FOOTER_LEN].try_into().unwrap());
    let actual_checksum = checksum(&bytes[..footer]);

    if actual_checksum != expected_checksum {
        return Err(storage_error("WAL checksum mismatch"));
    }

    let mut pages = BTreeMap::new();

    for index in 0..frame_count {
        let offset = HEADER_LEN + index * FRAME_LEN;
        let page_id = PageId(u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().unwrap(),
        ));
        let page_bytes: &[u8; PAGE_SIZE] =
            bytes[offset + 4..offset + FRAME_LEN].try_into().unwrap();
        let page = Page::decode(page_bytes)?;

        if pages.insert(page_id, page).is_some() {
            return Err(storage_error(format!(
                "WAL contains duplicate page {}",
                page_id.0
            )));
        }
    }

    let meta = pages
        .get(&PageId(0))
        .ok_or_else(|| storage_error("WAL transaction is missing the metadata page"))?;

    if meta.kind() != crate::page::PageKind::Meta || meta.auxiliary() != tx_id {
        return Err(storage_error("WAL transaction id does not match metadata"));
    }

    Ok(Some(WalTransaction { pages }))
}

fn storage_error(message: impl Into<String>) -> Error {
    Error::StorageMsg(format!("opfs-storage: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{file::tests::MemoryFile, page::PageKind},
    };

    fn sample_pages() -> BTreeMap<PageId, Page> {
        BTreeMap::from([
            (
                PageId(0),
                Page::new(PageKind::Meta, None, 7, b"meta".to_vec()).unwrap(),
            ),
            (
                PageId(1),
                Page::new(PageKind::RecordHead, None, 4, b"data".to_vec()).unwrap(),
            ),
        ])
    }

    #[test]
    fn recovers_committed_wal() {
        let database = MemoryFile::default();
        let wal = MemoryFile::default();
        let pages = sample_pages();
        let bytes = encode_transaction(7, &pages).unwrap();
        wal.write_at(0, &bytes).unwrap();

        recover(&database, &wal).unwrap();

        assert_eq!(wal.size().unwrap(), 0);
        assert_eq!(database.size().unwrap(), (PAGE_SIZE * 2) as u64);
        let mut actual = [0; PAGE_SIZE];
        database
            .read_exact_at(PageId(1).offset(), &mut actual)
            .unwrap();
        assert_eq!(Page::decode(&actual).unwrap(), pages[&PageId(1)]);
    }

    #[test]
    fn discards_uncommitted_wal() {
        let database = MemoryFile::default();
        let wal = MemoryFile::default();
        let pages = sample_pages();
        let mut bytes = encode_transaction(7, &pages).unwrap();
        bytes.truncate(bytes.len() - FOOTER_LEN);
        wal.write_at(0, &bytes).unwrap();

        recover(&database, &wal).unwrap();

        assert_eq!(database.size().unwrap(), 0);
        assert_eq!(wal.size().unwrap(), 0);
    }

    #[test]
    fn rejects_corrupt_committed_wal() {
        let pages = sample_pages();
        let mut bytes = encode_transaction(7, &pages).unwrap();
        bytes[HEADER_LEN + 10] ^= 0xff;

        assert!(decode_transaction(&bytes).is_err());
    }
}
