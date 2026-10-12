use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use securekey::crypto::{
    self, open_from_file, seal_to_writer, MasterKey, NopProgress, Progress, HEADER_LEN, TAG_LEN,
};
use securekey::vault::{classify_directory, create_vault, open_vault, CreateOptions, DirectoryKind};
use securekey::Error;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("securekey-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Pattern {
    pos: u64,
    len: u64,
}

impl Read for Pattern {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        let n = buf.len().min(usize::try_from(self.len - self.pos).unwrap_or(buf.len()));
        for (index, byte) in buf.iter_mut().enumerate().take(n) {
            *byte = ((self.pos + index as u64) % 251) as u8;
        }
        self.pos += n as u64;
        Ok(n)
    }
}

struct CancelAfter {
    reports: u32,
}

impl Progress for CancelAfter {
    fn report(&mut self, _done: u64, _total: Option<u64>) {
        self.reports += 1;
    }

    fn is_cancelled(&self) -> bool {
        self.reports >= 1
    }
}

fn password() -> &'static [u8] {
    b"phrase-de-test-solide"
}

fn vault() -> (TempDir, securekey::vault::VaultSession) {
    let dir = TempDir::new();
    let root = dir.path().join("Vault");
    let session = create_vault(&root, "Coffre test", password(), password(), CreateOptions::for_tests())
        .expect("création");
    (dir, session)
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
}

fn read_all(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

fn tree_contains(dir: &Path, needle: &str) -> bool {
    let needle = needle.as_bytes();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current).unwrap();
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if read_all(&path).windows(needle.len()).any(|window| window == needle) {
                return true;
            }
        }
    }
    false
}

#[test]
fn recommended_parameters_follow_rfc9106() {
    let params = crypto::KdfParams::recommended();
    assert_eq!(params.memory_kib, 64 * 1024);
    assert_eq!(params.iterations, 3);
    assert_eq!(params.parallelism, 4);
}

#[test]
fn create_open_and_reject_wrong_password() {
    let (dir, session) = vault();
    let root = session.paths().root().to_path_buf();
    drop(session);
    open_vault(&root, password(), false).expect("bon mot de passe");
    match open_vault(&root, b"mauvais-mot-de-passe", false) {
        Err(Error::AuthFailed) => {}
        Err(error) => panic!("{error}"),
        Ok(_) => panic!("un mauvais mot de passe a ouvert le coffre"),
    }
    assert!(dir.path().join("Vault/vault.meta").is_file());
}

#[test]
fn filenames_and_password_are_not_on_the_key() {
    securekey::platform::start_capture();
    let dir = TempDir::new();
    let root = dir.path().join("Vault");
    let mut session = create_vault(
        &root,
        "Coffre",
        b"CANARY-PASSWORD-9f3aXX",
        b"CANARY-PASSWORD-9f3aXX",
        CreateOptions::for_tests(),
    )
    .unwrap();
    let source = dir.path().join("CANARY-passeport-α.pdf");
    write_bytes(&source, b"UNIQUE-PLAINTEXT-xyz");
    session
        .import_file(&source, None, &mut NopProgress)
        .unwrap();
    let logs = securekey::platform::stop_capture();
    assert!(!logs.contains("CANARY-PASSWORD"));
    assert!(!logs.contains("CANARY-passeport"));
    assert!(!logs.contains("UNIQUE-PLAINTEXT"));
    assert!(!tree_contains(&root, "CANARY-PASSWORD-9f3aXX"));
    assert!(!tree_contains(&root, "CANARY-passeport"));
    assert!(!tree_contains(&root, "UNIQUE-PLAINTEXT-xyz"));
    let objects = fs::read_dir(root.join("objects")).unwrap();
    for entry in objects {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(name.ends_with(".dat"));
        assert_eq!(name.len(), 32 + 4);
    }
}

#[test]
fn empty_small_and_multichunk_roundtrip() {
    let (dir, mut session) = vault();
    let empty = dir.path().join("vide.bin");
    let small = dir.path().join("petit.bin");
    let multi = dir.path().join("blocs.bin");
    write_bytes(&empty, b"");
    write_bytes(&small, b"bonjour le coffre");
    let mut multi_bytes = vec![0u8; 4096 * 2 + 10];
    for (index, byte) in multi_bytes.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    write_bytes(&multi, &multi_bytes);

    let empty_id = session.import_file(&empty, None, &mut NopProgress).unwrap();
    let small_id = session.import_file(&small, None, &mut NopProgress).unwrap();
    let multi_id = session.import_file(&multi, None, &mut NopProgress).unwrap();

    let object = session.paths().objects_dir();
    let mut lengths = fs::read_dir(&object)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .collect::<Vec<_>>();
    lengths.sort();
    assert_eq!(
        lengths,
        vec![
            expected_len(0, 4096),
            expected_len(b"bonjour le coffre".len() as u64, 4096),
            expected_len(multi_bytes.len() as u64, 4096),
        ]
    );

    let export_dir = dir.path().join("out");
    fs::create_dir(&export_dir).unwrap();
    session
        .export_entries(
            &[empty_id.id.clone(), small_id.id.clone(), multi_id.id],
            &export_dir,
            &mut NopProgress,
        )
        .unwrap();
    assert_eq!(read_all(&export_dir.join("vide.bin")), b"");
    assert_eq!(read_all(&export_dir.join("petit.bin")), b"bonjour le coffre");
    assert_eq!(read_all(&export_dir.join("blocs.bin")), multi_bytes);
}

fn expected_len(plain: u64, chunk: u64) -> u64 {
    let full = plain / chunk;
    let remainder = plain % chunk;
    let mut cipher = HEADER_LEN as u64;
    if plain == 0 {
        return cipher + TAG_LEN as u64;
    }
    if remainder == 0 {
        cipher += full * (chunk + TAG_LEN as u64);
    } else {
        cipher += full * (chunk + TAG_LEN as u64);
        cipher += remainder + TAG_LEN as u64;
    }
    cipher
}

#[test]
fn unicode_long_names_and_identical_files() {
    let (dir, mut session) = vault();
    let french = dir.path().join("impôt_2026.pdf");
    let japanese = dir.path().join("税金.txt");
    let short = dir.path().join("court.txt");
    write_bytes(&french, b"fr");
    write_bytes(&japanese, b"jp");
    write_bytes(&short, b"long");
    session.import_file(&french, None, &mut NopProgress).unwrap();
    session.import_file(&japanese, None, &mut NopProgress).unwrap();
    let long_entry = session.import_file(&short, None, &mut NopProgress).unwrap();
    let long_name = "あ".repeat(255);
    session.rename_entry(&long_entry.id, &long_name).unwrap();
    let names: Vec<_> = session
        .list("folder", None)
        .unwrap()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert!(names.iter().any(|name| name == "impôt_2026.pdf"));
    assert!(names.iter().any(|name| name == "税金.txt"));
    assert!(names.iter().any(|name| name == &long_name));
    let too_long = "a".repeat(256);
    match session.rename_entry(&long_entry.id, &too_long) {
        Err(Error::InvalidName) => {}
        other => panic!("nom trop long accepté : {other:?}"),
    }

    let copy_a = dir.path().join("meme-a.bin");
    let copy_b = dir.path().join("meme-b.bin");
    write_bytes(&copy_a, b"same-bytes");
    write_bytes(&copy_b, b"same-bytes");
    session.import_file(&copy_a, None, &mut NopProgress).unwrap();
    session.import_file(&copy_b, None, &mut NopProgress).unwrap();
    let twins: Vec<_> = fs::read_dir(session.paths().objects_dir())
        .unwrap()
        .map(|entry| read_all(&entry.unwrap().path()))
        .filter(|bytes| bytes.len() == expected_len(b"same-bytes".len() as u64, 4096) as usize)
        .collect();
    assert_eq!(twins.len(), 2);
    assert_ne!(twins[0], twins[1]);
}

#[test]
fn corruption_is_detected_and_does_not_rewrite_the_vault() {
    let (dir, mut session) = vault();
    let source = dir.path().join("note.txt");
    write_bytes(&source, b"contenu stable");
    session.import_file(&source, None, &mut NopProgress).unwrap();
    let meta_before = read_all(&session.paths().meta());
    let index_before = read_all(&session.paths().index());
    let object = fs::read_dir(session.paths().objects_dir())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = read_all(&object);
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    write_bytes(&object, &bytes);
    let export_dir = dir.path().join("export");
    fs::create_dir(&export_dir).unwrap();
    let id = session.list("folder", None).unwrap()[0].id.clone();
    assert!(matches!(
        session.export_entries(&[id], &export_dir, &mut NopProgress).unwrap_err(),
        Error::AuthFailed
    ));
    assert_eq!(read_all(&session.paths().meta()), meta_before);
    assert_eq!(read_all(&session.paths().index()), index_before);
    assert!(export_dir.read_dir().unwrap().next().is_none());

    let root = session.paths().root().to_path_buf();
    drop(session);
    let mut index = read_all(&root.join("index.enc"));
    let middle = index.len() / 2;
    index[middle] ^= 0x5a;
    write_bytes(&root.join("index.enc"), &index);
    match open_vault(&root, password(), false) {
        Err(Error::Damaged) => {}
        Err(error) => panic!("{error}"),
        Ok(_) => panic!("un index corrompu a été accepté"),
    }
}

#[test]
fn interrupted_write_is_not_a_valid_object() {
    let (_dir, mut session) = vault();
    let source_dir = TempDir::new();
    let source = source_dir.path().join("gros.bin");
    write_bytes(&source, &vec![9u8; 9000]);
    let error = session
        .import_file(&source, None, &mut CancelAfter { reports: 0 })
        .unwrap_err();
    assert!(matches!(error, Error::Cancelled));
    assert!(session.list("folder", None).unwrap().is_empty());
    let names = fs::read_dir(session.paths().objects_dir()).unwrap();
    for entry in names {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(name.starts_with('.'), "{name}");
    }
    write_bytes(
        &session.paths().objects_dir().join(".abcdef.dat.tmp-deadbeef"),
        b"incomplet",
    );
    let report = session.verify().unwrap();
    assert!(report.issues.iter().any(|issue| issue.code == "object.temporary"));
    assert!(session.list("folder", None).unwrap().is_empty());
}

#[test]
fn password_change_keeps_file_bytes() {
    let (dir, mut session) = vault();
    let source = dir.path().join("photo.jpg");
    write_bytes(&source, b"image-bytes");
    let imported = session.import_file(&source, None, &mut NopProgress).unwrap();
    let object = fs::read_dir(session.paths().objects_dir())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let before = read_all(&object);
    session
        .change_password(password(), b"autre-phrase-secrete", b"autre-phrase-secrete")
        .unwrap();
    assert_eq!(read_all(&object), before);
    let root = session.paths().root().to_path_buf();
    drop(session);
    match open_vault(&root, password(), false) {
        Err(Error::AuthFailed) => {}
        Err(error) => panic!("{error}"),
        Ok(_) => panic!("l'ancien mot de passe fonctionne encore"),
    }
    let reopened = open_vault(&root, b"autre-phrase-secrete", false).unwrap();
    let export_dir = dir.path().join("out");
    fs::create_dir(&export_dir).unwrap();
    reopened
        .export_entries(&[imported.id], &export_dir, &mut NopProgress)
        .unwrap();
    assert_eq!(read_all(&export_dir.join("photo.jpg")), b"image-bytes");
}

#[test]
fn rename_delete_folders_and_read_only() {
    let (dir, mut session) = vault();
    let folder = session.create_folder(None, "Famille").unwrap();
    let source = dir.path().join("note.txt");
    write_bytes(&source, b"abc");
    let file = session
        .import_file(&source, Some(&folder.id), &mut NopProgress)
        .unwrap();
    session.rename_entry(&file.id, "lettre.txt").unwrap();
    let names: Vec<_> = session
        .list("folder", Some(&folder.id))
        .unwrap()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(names, vec!["lettre.txt".to_string()]);
    session.trash_entries(&[folder.id.clone()]).unwrap();
    assert!(session.list("folder", None).unwrap().is_empty());
    assert_eq!(session.list("trash", None).unwrap().len(), 1);
    session.restore_entries(&[folder.id.clone()]).unwrap();
    session.delete_permanent(&[folder.id]).unwrap();
    assert!(session.list("folder", None).unwrap().is_empty());
    assert!(fs::read_dir(session.paths().objects_dir()).unwrap().next().is_none());

    let root = session.paths().root().to_path_buf();
    let index = read_all(&session.paths().index());
    drop(session);
    let mut readonly = open_vault(&root, password(), true).unwrap();
    let fresh = dir.path().join("autre.txt");
    write_bytes(&fresh, b"non");
    assert!(matches!(
        readonly.import_file(&fresh, None, &mut NopProgress).unwrap_err(),
        Error::ReadOnly
    ));
    assert_eq!(read_all(&readonly.paths().index()), index);
}

#[test]
fn backup_roundtrip_and_header_layout() {
    let (dir, mut session) = vault();
    let source = dir.path().join("archive.bin");
    write_bytes(&source, b"archive");
    session.import_file(&source, None, &mut NopProgress).unwrap();
    let backup_parent = dir.path().join("backups");
    fs::create_dir(&backup_parent).unwrap();
    let copied = securekey::vault::backup_vault(session.paths(), &backup_parent, &mut NopProgress).unwrap();
    let opened = open_vault(&copied, password(), true).unwrap();
    let export_dir = dir.path().join("from-backup");
    fs::create_dir(&export_dir).unwrap();
    let id = opened.list("folder", None).unwrap()[0].id.clone();
    opened
        .export_entries(&[id], &export_dir, &mut NopProgress)
        .unwrap();
    assert_eq!(read_all(&export_dir.join("archive.bin")), b"archive");

    let object = fs::read_dir(session.paths().objects_dir())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = read_all(&object);
    assert_eq!(&bytes[0..4], b"SKO1");
    assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(bytes[6..10].try_into().unwrap()), 4096);
    assert_eq!(bytes[10], 19);
    let meta = String::from_utf8(read_all(&session.paths().meta())).unwrap();
    assert!(meta.contains("\"format_version\": 1"));
    assert!(meta.contains("\"algorithm\": \"argon2id\""));
    assert!(meta.contains("\"algorithm\": \"xchacha20poly1305\""));
    assert!(!meta.contains('\\'));
    assert!(!meta.contains("recovery"));
    assert!(matches!(
        crypto::create_recovery_key().unwrap_err(),
        Error::RecoveryUnavailable
    ));
}

#[test]
fn streaming_ciphertext_has_the_documented_size() {
    let dir = TempDir::new();
    let plain_len = 4096 * 2 + 10;
    let key = MasterKey::new([11u8; 32]);
    let vault_id = [2u8; 16];
    let object_id = [3u8; 16];
    let path = dir.path().join("objet.dat");
    {
        let mut input = Pattern {
            pos: 0,
            len: plain_len,
        };
        let file = File::create(&path).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        seal_to_writer(
            &key,
            &crypto::object_aad(&vault_id, &object_id),
            &mut input,
            &mut writer,
            4096,
            Some(plain_len),
            &mut NopProgress,
        )
        .unwrap();
        writer.flush().unwrap();
    }
    assert_eq!(fs::metadata(&path).unwrap().len(), expected_len(plain_len, 4096));
    let mut file = File::open(&path).unwrap();
    let mut output = Vec::new();
    open_from_file(
        &key,
        &crypto::object_aad(&vault_id, &object_id),
        &mut file,
        &mut output,
        &mut NopProgress,
    )
    .unwrap();
    assert_eq!(output.len() as u64, plain_len);
    assert_eq!(output[0], 0);
    assert_eq!(output[plain_len as usize - 1], ((plain_len - 1) % 251) as u8);
}

#[test]
#[ignore = "écrit environ 2 Gio ; cargo test -- --ignored streams_multiple_gigabytes"]
fn streams_multiple_gigabytes() {
    let dir = TempDir::new();
    let plain_len = 2 * 1024 * 1024 * 1024 + 100;
    let key = MasterKey::new([5u8; 32]);
    let aad = b"securekey-large".as_slice();
    let path = dir.path().join("large.dat");
    let mut input = Pattern {
        pos: 0,
        len: plain_len,
    };
    let file = File::create(&path).unwrap();
    let mut writer = std::io::BufWriter::new(file);
    seal_to_writer(&key, aad, &mut input, &mut writer, 1024 * 1024, Some(plain_len), &mut NopProgress)
        .unwrap();
    writer.flush().unwrap();
    let mut file = File::open(&path).unwrap();
    let mut writer = CompareWriter {
        pattern: Pattern {
            pos: 0,
            len: plain_len,
        },
    };
    open_from_file(&key, aad, &mut file, &mut writer, &mut NopProgress).unwrap();
    assert_eq!(writer.pattern.pos, plain_len);
}

struct CompareWriter {
    pattern: Pattern,
}

impl Write for CompareWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut expected = vec![0u8; buf.len()];
        let n = self.pattern.read(&mut expected)?;
        assert_eq!(&expected[..n], buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn temporary_open_can_be_reimported_and_wiped() {
    let (dir, mut session) = vault();
    let source = dir.path().join("brouillon.txt");
    write_bytes(&source, b"version 1");
    let imported = session.import_file(&source, None, &mut NopProgress).unwrap();
    let temp = session.prepare_temp(&imported.id, &mut NopProgress).unwrap();
    assert!(temp.starts_with(std::env::temp_dir()));
    assert!(!temp.starts_with(session.paths().root()));
    write_bytes(&temp, b"version 2, plus longue");
    assert!(session.temp_status().unwrap()[0].changed);
    session.reimport_temp(&imported.id, &mut NopProgress).unwrap();
    assert!(session.temp_status().unwrap().is_empty());
    let export_dir = dir.path().join("out");
    fs::create_dir(&export_dir).unwrap();
    session
        .export_entries(&[imported.id], &export_dir, &mut NopProgress)
        .unwrap();
    assert_eq!(read_all(&export_dir.join("brouillon.txt")), b"version 2, plus longue");
}

#[test]
fn an_empty_objects_folder_becomes_a_vault() {
    let dir = TempDir::new();
    let vault = dir.path().join("Vault");
    fs::create_dir_all(vault.join("objects")).unwrap();
    fs::write(vault.join("._objects"), b"appledouble").unwrap();
    assert_eq!(classify_directory(&vault), DirectoryKind::Blank);
    create_vault(&vault, "Nom", password(), password(), CreateOptions::for_tests()).unwrap();
    assert!(vault.join("vault.meta").is_file());
    assert_eq!(classify_directory(&vault), DirectoryKind::Vault);
}

#[test]
fn a_folder_that_already_holds_files_is_left_alone() {
    let dir = TempDir::new();
    fs::write(dir.path().join("note.txt"), b"x").unwrap();
    assert_eq!(classify_directory(dir.path()), DirectoryKind::Occupied);
    match create_vault(dir.path(), "Nom", password(), password(), CreateOptions::for_tests()) {
        Err(Error::AlreadyExists) => {}
        Err(error) => panic!("mauvaise erreur : {error}"),
        Ok(_) => panic!("coffre créé dans un dossier occupé"),
    }
}

#[test]
fn master_key_debug_is_redacted() {
    let key = MasterKey::new([0xab; 32]);
    let rendered = format!("{key:?}");
    assert!(!rendered.to_lowercase().contains("ab"));
    assert!(rendered.contains("redacted"));
}
