use {
    crate::{
        file::RandomAccessFile,
        page::{PAGE_PAYLOAD_SIZE, PAGE_SIZE, Page, PageId, PageKind},
        wal,
    },
    gluesql_core::error::{Error, Result},
    std::collections::{BTreeMap, BTreeSet},
};

// Page zero stores metadata. Every persisted schema or row starts at a RecordHead page and may
// continue through RecordBody pages. This first format intentionally stores one logical record per
// page chain; packing multiple records into a slotted page can be added without changing Store.
const META_MAGIC: [u8; 4] = *b"GLDB";
const STORAGE_VERSION: u32 = 1;
const META_PAYLOAD_LEN: usize = 32;
const NONE_PAGE_ID: u32 = u32::MAX;
const META_PAGE_ID: PageId = PageId(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Meta {
    tx_id: u64,
    page_count: u32,
    freelist_head: Option<PageId>,
}

impl Meta {
    fn empty() -> Self {
        Self {
            tx_id: 0,
            page_count: 1,
            freelist_head: None,
        }
    }

    fn to_page(self) -> Result<Page> {
        let mut payload = vec![0; META_PAYLOAD_LEN];
        payload[..4].copy_from_slice(&META_MAGIC);
        payload[4..8].copy_from_slice(&STORAGE_VERSION.to_le_bytes());
        payload[8..12].copy_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
        payload[12..16].copy_from_slice(&self.page_count.to_le_bytes());
        payload[16..20].copy_from_slice(
            &self
                .freelist_head
                .map_or(NONE_PAGE_ID, |page_id| page_id.0)
                .to_le_bytes(),
        );
        payload[24..32].copy_from_slice(&self.tx_id.to_le_bytes());

        Page::new(PageKind::Meta, None, self.tx_id, payload)
    }

    fn from_page(page: &Page) -> Result<Self> {
        if page.kind() != PageKind::Meta {
            return Err(storage_error("page 0 is not a metadata page"));
        }

        let payload = page.payload();

        if payload.len() != META_PAYLOAD_LEN {
            return Err(storage_error("metadata payload has an invalid length"));
        }

        if payload[..4] != META_MAGIC {
            return Err(storage_error("invalid storage magic"));
        }

        let version = u32::from_le_bytes(payload[4..8].try_into().unwrap());

        if version != STORAGE_VERSION {
            return Err(storage_error(format!(
                "unsupported storage version {version}, expected {STORAGE_VERSION}"
            )));
        }

        let page_size = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;

        if page_size != PAGE_SIZE {
            return Err(storage_error(format!(
                "unsupported page size {page_size}, expected {PAGE_SIZE}"
            )));
        }

        let page_count = u32::from_le_bytes(payload[12..16].try_into().unwrap());

        if page_count == 0 {
            return Err(storage_error("metadata page count is zero"));
        }

        let freelist_head = u32::from_le_bytes(payload[16..20].try_into().unwrap());
        let freelist_head = (freelist_head != NONE_PAGE_ID).then_some(PageId(freelist_head));
        let tx_id = u64::from_le_bytes(payload[24..32].try_into().unwrap());

        if page.auxiliary() != tx_id {
            return Err(storage_error("metadata transaction id mismatch"));
        }

        Ok(Self {
            tx_id,
            page_count,
            freelist_head,
        })
    }
}

pub struct Pager<D, W> {
    database: D,
    wal: W,
    meta: Meta,
}

impl<D, W> Pager<D, W>
where
    D: RandomAccessFile,
    W: RandomAccessFile,
{
    pub fn open(database: D, wal_file: W) -> Result<Self> {
        wal::recover(&database, &wal_file)?;

        let meta = if database.size()? == 0 {
            let meta = Meta::empty();
            let pages = BTreeMap::from([(META_PAGE_ID, meta.to_page()?)]);
            wal::commit(&database, &wal_file, meta.tx_id, &pages)?;
            meta
        } else {
            let size = database.size()?;

            if size % PAGE_SIZE as u64 != 0 {
                return Err(storage_error("database file is not page-aligned"));
            }

            let mut bytes = [0; PAGE_SIZE];
            database.read_exact_at(0, &mut bytes)?;
            let meta = Meta::from_page(&Page::decode(&bytes)?)?;
            let expected_size = u64::from(meta.page_count) * PAGE_SIZE as u64;

            if size != expected_size {
                return Err(storage_error(format!(
                    "database size {size} does not match metadata size {expected_size}"
                )));
            }

            meta
        };

        let pager = Self {
            database,
            wal: wal_file,
            meta,
        };
        pager.validate()?;

        Ok(pager)
    }

    pub fn transaction(&mut self) -> PageTransaction<'_, D, W> {
        let meta = self.meta;

        PageTransaction {
            pager: self,
            meta,
            dirty_pages: BTreeMap::new(),
        }
    }

    pub fn records(&self) -> Result<Vec<(PageId, Vec<u8>)>> {
        let pages = self.read_all_data_pages()?;
        let free_pages = self.validate_freelist(&pages)?;
        let mut used_bodies = BTreeSet::new();
        let mut records = Vec::new();

        for (page_id, page) in &pages {
            match page.kind() {
                PageKind::RecordHead => {
                    let (record, bodies) = collect_record(*page_id, &pages)?;

                    for body in bodies {
                        if !used_bodies.insert(body) {
                            return Err(storage_error(format!(
                                "record body page {} is referenced more than once",
                                body.0
                            )));
                        }
                    }

                    records.push((*page_id, record));
                }
                PageKind::RecordBody => {}
                PageKind::Free => {
                    if !free_pages.contains(page_id) {
                        return Err(storage_error(format!(
                            "free page {} is missing from the freelist",
                            page_id.0
                        )));
                    }
                }
                PageKind::Meta => {
                    return Err(storage_error(format!(
                        "unexpected metadata page {}",
                        page_id.0
                    )));
                }
            }
        }

        for (page_id, page) in &pages {
            if page.kind() == PageKind::RecordBody && !used_bodies.contains(page_id) {
                return Err(storage_error(format!(
                    "record body page {} is not referenced",
                    page_id.0
                )));
            }
        }

        Ok(records)
    }

    #[cfg(test)]
    fn page_count(&self) -> u32 {
        self.meta.page_count
    }

    fn read_page(&self, page_id: PageId) -> Result<Page> {
        if page_id.0 >= self.meta.page_count {
            return Err(storage_error(format!("page {} is out of range", page_id.0)));
        }

        let mut bytes = [0; PAGE_SIZE];
        self.database.read_exact_at(page_id.offset(), &mut bytes)?;
        Page::decode(&bytes)
    }

    fn read_all_data_pages(&self) -> Result<BTreeMap<PageId, Page>> {
        (1..self.meta.page_count)
            .map(|page_id| {
                let page_id = PageId(page_id);
                self.read_page(page_id).map(|page| (page_id, page))
            })
            .collect()
    }

    fn validate_freelist(&self, pages: &BTreeMap<PageId, Page>) -> Result<BTreeSet<PageId>> {
        let mut free_pages = BTreeSet::new();
        let mut current = self.meta.freelist_head;

        while let Some(page_id) = current {
            if !free_pages.insert(page_id) {
                return Err(storage_error("freelist contains a cycle"));
            }

            let page = pages.get(&page_id).ok_or_else(|| {
                storage_error(format!("freelist page {} is out of range", page_id.0))
            })?;

            if page.kind() != PageKind::Free {
                return Err(storage_error(format!(
                    "freelist page {} is not marked free",
                    page_id.0
                )));
            }

            current = page.next();
        }

        Ok(free_pages)
    }

    fn validate(&self) -> Result<()> {
        self.records().map(|_| ())
    }
}

pub struct PageTransaction<'a, D, W> {
    pager: &'a mut Pager<D, W>,
    meta: Meta,
    dirty_pages: BTreeMap<PageId, Page>,
}

impl<D, W> PageTransaction<'_, D, W>
where
    D: RandomAccessFile,
    W: RandomAccessFile,
{
    pub fn write_record(&mut self, bytes: &[u8]) -> Result<PageId> {
        let page_count = bytes.len().div_ceil(PAGE_PAYLOAD_SIZE).max(1);
        let page_ids = (0..page_count)
            .map(|_| self.allocate_page())
            .collect::<Result<Vec<_>>>()?;
        let total_len = bytes.len() as u64;

        for (index, page_id) in page_ids.iter().enumerate() {
            let start = index * PAGE_PAYLOAD_SIZE;
            let end = (start + PAGE_PAYLOAD_SIZE).min(bytes.len());
            let payload = if start < bytes.len() {
                bytes[start..end].to_vec()
            } else {
                Vec::new()
            };
            let next = page_ids.get(index + 1).copied();
            let kind = if index == 0 {
                PageKind::RecordHead
            } else {
                PageKind::RecordBody
            };
            let auxiliary = if index == 0 { total_len } else { 0 };
            let page = Page::new(kind, next, auxiliary, payload)?;
            self.dirty_pages.insert(*page_id, page);
        }

        Ok(page_ids[0])
    }

    pub fn free_record(&mut self, head: PageId) -> Result<()> {
        let mut current = Some(head);
        let mut visited = BTreeSet::new();
        let mut first = true;

        while let Some(page_id) = current {
            if !visited.insert(page_id) {
                return Err(storage_error("record page chain contains a cycle"));
            }

            let page = self.read_page(page_id)?;
            let expected = if first {
                PageKind::RecordHead
            } else {
                PageKind::RecordBody
            };

            if page.kind() != expected {
                return Err(storage_error(format!(
                    "record page {} has unexpected kind {:?}",
                    page_id.0,
                    page.kind()
                )));
            }

            current = page.next();
            let free_page = Page::free(self.meta.freelist_head);
            self.meta.freelist_head = Some(page_id);
            self.dirty_pages.insert(page_id, free_page);
            first = false;
        }

        Ok(())
    }

    pub fn commit(mut self) -> Result<()> {
        if self.dirty_pages.is_empty() {
            return Ok(());
        }

        self.meta.tx_id = self
            .meta
            .tx_id
            .checked_add(1)
            .ok_or_else(|| storage_error("transaction id overflow"))?;
        self.dirty_pages.insert(META_PAGE_ID, self.meta.to_page()?);
        wal::commit(
            &self.pager.database,
            &self.pager.wal,
            self.meta.tx_id,
            &self.dirty_pages,
        )?;
        self.pager.meta = self.meta;

        Ok(())
    }

    fn allocate_page(&mut self) -> Result<PageId> {
        if let Some(page_id) = self.meta.freelist_head {
            let page = self.read_page(page_id)?;

            if page.kind() != PageKind::Free {
                return Err(storage_error(format!(
                    "freelist page {} is not marked free",
                    page_id.0
                )));
            }

            self.meta.freelist_head = page.next();

            Ok(page_id)
        } else {
            let page_id = PageId(self.meta.page_count);
            self.meta.page_count = self
                .meta
                .page_count
                .checked_add(1)
                .ok_or_else(|| storage_error("page id overflow"))?;

            Ok(page_id)
        }
    }

    fn read_page(&self, page_id: PageId) -> Result<Page> {
        if let Some(page) = self.dirty_pages.get(&page_id) {
            return Ok(page.clone());
        }

        if page_id.0 >= self.pager.meta.page_count {
            return Err(storage_error(format!("page {} is out of range", page_id.0)));
        }

        self.pager.read_page(page_id)
    }
}

fn collect_record(
    head: PageId,
    pages: &BTreeMap<PageId, Page>,
) -> Result<(Vec<u8>, BTreeSet<PageId>)> {
    let head_page = pages
        .get(&head)
        .ok_or_else(|| storage_error(format!("record head page {} is missing", head.0)))?;
    let expected_len = usize::try_from(head_page.auxiliary())
        .map_err(|_| storage_error("record is too large to load"))?;
    let mut bytes = Vec::with_capacity(expected_len);
    bytes.extend_from_slice(head_page.payload());
    let mut bodies = BTreeSet::new();
    let mut current = head_page.next();

    while let Some(page_id) = current {
        if !bodies.insert(page_id) {
            return Err(storage_error("record page chain contains a cycle"));
        }

        let page = pages.get(&page_id).ok_or_else(|| {
            storage_error(format!("record body page {} is out of range", page_id.0))
        })?;

        if page.kind() != PageKind::RecordBody {
            return Err(storage_error(format!(
                "record body page {} has unexpected kind {:?}",
                page_id.0,
                page.kind()
            )));
        }

        bytes.extend_from_slice(page.payload());
        current = page.next();
    }

    if bytes.len() != expected_len {
        return Err(storage_error(format!(
            "record length mismatch: {} != {expected_len}",
            bytes.len()
        )));
    }

    Ok((bytes, bodies))
}

fn storage_error(message: impl Into<String>) -> Error {
    Error::StorageMsg(format!("opfs-storage: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::file::tests::MemoryFile};

    #[test]
    fn writes_and_reopens_records() {
        let database = MemoryFile::default();
        let wal = MemoryFile::default();
        let mut pager = Pager::open(database.clone(), wal.clone()).unwrap();
        let mut transaction = pager.transaction();
        let head = transaction.write_record(b"hello").unwrap();
        transaction.commit().unwrap();

        assert_eq!(pager.records().unwrap(), vec![(head, b"hello".to_vec())]);

        let pager = Pager::open(database, wal).unwrap();
        assert_eq!(pager.records().unwrap(), vec![(head, b"hello".to_vec())]);
    }

    #[test]
    fn stores_records_across_multiple_pages() {
        let database = MemoryFile::default();
        let wal = MemoryFile::default();
        let mut pager = Pager::open(database, wal).unwrap();
        let expected = vec![7; PAGE_PAYLOAD_SIZE * 2 + 17];
        let mut transaction = pager.transaction();
        let head = transaction.write_record(&expected).unwrap();
        transaction.commit().unwrap();

        assert_eq!(pager.records().unwrap(), vec![(head, expected)]);
        assert_eq!(pager.page_count(), 4);
    }

    #[test]
    fn reuses_freed_pages_without_growing_forever() {
        let database = MemoryFile::default();
        let wal = MemoryFile::default();
        let mut pager = Pager::open(database.clone(), wal.clone()).unwrap();
        let mut transaction = pager.transaction();
        let mut head = transaction.write_record(b"version-0").unwrap();
        transaction.commit().unwrap();

        for version in 1..100 {
            let mut transaction = pager.transaction();
            transaction.free_record(head).unwrap();
            let next = transaction
                .write_record(format!("version-{version}").as_bytes())
                .unwrap();
            transaction.commit().unwrap();
            head = next;
        }

        assert_eq!(pager.page_count(), 2);
        assert_eq!(database.size().unwrap(), (PAGE_SIZE * 2) as u64);

        let mut transaction = pager.transaction();
        transaction.free_record(head).unwrap();
        transaction.commit().unwrap();

        drop(pager);
        let mut pager = Pager::open(database, wal).unwrap();
        let mut transaction = pager.transaction();
        let reused = transaction.write_record(b"reused").unwrap();
        transaction.commit().unwrap();

        assert_eq!(reused, head);
        assert_eq!(pager.page_count(), 2);
    }
}
