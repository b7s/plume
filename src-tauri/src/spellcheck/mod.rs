use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

/// Returns the LibreOffice dictionaries URL for a given language code.
/// Covers the most common languages where the directory differs from the filename.
/// Falls back to `{lang}/{lang}` for unmapped languages.
pub fn dict_url(language: &str) -> String {
    let base = "https://raw.githubusercontent.com/LibreOffice/dictionaries/master";
    // (language code, subdirectory, filename)
    let special: &[(&str, &str)] = &[
        ("en_US", "en"),
        ("en_GB", "en"),
        ("es_ES", "es"),
        ("es_MX", "es"),
        ("es_AR", "es"),
        ("fr_FR", "fr"),
        ("de_DE", "de"),
        ("de_CH", "de"),
        ("it_IT", "it"),
        ("nl_NL", "nl"),
        ("pl_PL", "pl"),
        ("ru_RU", "ru"),
        ("uk_UA", "uk"),
        ("sv_SE", "sv"),
        ("da_DK", "da"),
        ("fi_FI", "fi"),
        ("nb_NO", "no"),
        ("nn_NO", "no"),
        ("cs_CZ", "cs"),
        ("el_GR", "el"),
        ("tr_TR", "tr"),
        ("hu_HU", "hu"),
        ("ro_RO", "ro"),
        ("bg_BG", "bg"),
        ("hr_HR", "hr"),
        ("sk_SK", "sk"),
        ("sl_SI", "sl"),
        ("lv_LV", "lv"),
        ("lt_LT", "lt"),
        ("et_EE", "et"),
        ("ca_ES", "ca"),
        ("gl_ES", "gl"),
        ("eu_ES", "eu"),
        ("he_IL", "he"),
        ("ar", "ar"),
        ("ja_JP", "ja"),
        ("ko_KR", "ko"),
        ("zh_CN", "zh_CN"),
        ("zh_TW", "zh_TW"),
        ("th_TH", "th"),
        ("vi_VN", "vi"),
        ("id_ID", "id"),
        ("ms_MY", "ms"),
    ];

    for &(lang, dir) in special {
        if lang == language {
            return format!("{base}/{dir}/{language}");
        }
    }

    // Default: directory matches language code
    format!("{base}/{language}/{language}")
}

/// Lists supported languages with display names.
pub fn available_languages() -> Vec<(&'static str, &'static str)> {
    vec![
        ("en_US", "English (US)"),
        ("en_GB", "English (UK)"),
        ("pt_BR", "Português (Brasil)"),
        ("pt_PT", "Português (Portugal)"),
        ("es_ES", "Español (España)"),
        ("es_MX", "Español (México)"),
        ("fr_FR", "Français"),
        ("de_DE", "Deutsch"),
        ("it_IT", "Italiano"),
        ("nl_NL", "Nederlands"),
        ("ru_RU", "Русский"),
        ("uk_UA", "Українська"),
        ("pl_PL", "Polski"),
        ("sv_SE", "Svenska"),
        ("da_DK", "Dansk"),
        ("fi_FI", "Suomi"),
        ("nb_NO", "Norsk (Bokmål)"),
        ("nn_NO", "Norsk (Nynorsk)"),
        ("cs_CZ", "Čeština"),
        ("el_GR", "Ελληνικά"),
        ("tr_TR", "Türkçe"),
        ("hu_HU", "Magyar"),
        ("ro_RO", "Română"),
        ("bg_BG", "Български"),
        ("hr_HR", "Hrvatski"),
        ("sk_SK", "Slovenčina"),
        ("sl_SI", "Slovenščina"),
        ("ca_ES", "Català"),
        ("ja_JP", "日本語"),
        ("ko_KR", "한국어"),
        ("zh_CN", "中文 (简体)"),
        ("zh_TW", "中文 (繁體)"),
        ("ar", "العربية"),
        ("he_IL", "עברית"),
        ("th_TH", "ไทย"),
        ("vi_VN", "Tiếng Việt"),
        ("id_ID", "Bahasa Indonesia"),
    ]
}

/// Very common English words that should appear first in prefix matches.
const COMMON_WORDS: &str = "
the be to of and a in that have I it for not on with he as you do at this but his by from they we say her she or an will my one all would there their what so up out if about who get which go me when make can like time no just him know take people into year your good some could them see other than then now look only come its over think also back after use two how our work first well way even new want because any these give day most us
is are was were has had did does done going being am got made make makes making takes took taken gets got got find found found give gives given giving go goes went gone come comes coming do does doing did done have has had having see sees saw seen seeing know knows knew known knowing think thinks thought thinking want wants wanted wanting use uses used using work works worked working look looks looked looking find finds found finding go goes gone going take takes took taking make makes made making come comes came coming get gets got getting give gives gave giving
process program project problem provide product profile property protocol promote proposal proceed procedure progress protect promise proof proper protocol proportion propose provision public publish purpose purchase pursue put point part party pay particular past pattern peace people perform period person phase phone picture piece place plan plant play player please point policy political pool poor population position positive possible power practice prepare present press pressure pretty prevent price primary prime principle print prior private problem procedure process produce product production professional program project promise proof proper property propose protect prove provide public publish purpose
";

struct Dict {
    set: HashSet<String>,
    sorted: Vec<String>,
    common: HashSet<String>,
}

static DICT: Mutex<Option<Dict>> = Mutex::new(None);

const LETTERS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '\'',
];

fn dict_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("plume").join("dictionaries")
    } else {
        PathBuf::from("dictionaries")
    }
}

pub fn aff_path(language: &str) -> PathBuf {
    dict_dir().join(format!("{language}.aff"))
}

pub fn dic_path(language: &str) -> PathBuf {
    dict_dir().join(format!("{language}.dic"))
}

pub fn init(language: &str) -> Result<(), String> {
    let dic = dic_path(language);
    let data = std::fs::read_to_string(&dic)
        .map_err(|e| format!("Failed to read {dic:?}: {e}"))?;

    let mut set = HashSet::new();
    let mut sorted = Vec::new();
    let common: HashSet<String> = COMMON_WORDS
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();

    for line in data.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Strip affix flags: "word/flag1,flag2" → "word"
        let word = line.split('/').next().unwrap_or(line).to_lowercase();
        if !word.is_empty() {
            set.insert(word.clone());
            sorted.push(word);
        }
    }

    sorted.sort();

    let mut guard = DICT.lock().map_err(|e| format!("Lock error: {e}"))?;
    *guard = Some(Dict { set, sorted, common });
    eprintln!("[plume] Spellcheck loaded {count} words", count = guard.as_ref().map(|d| d.set.len()).unwrap_or(0));
    Ok(())
}

pub fn is_initialized() -> bool {
    DICT.lock().ok().is_some_and(|g| g.is_some())
}

pub fn check(word: &str) -> Result<bool, String> {
    let guard = DICT.lock().map_err(|e| format!("Lock error: {e}"))?;
    match guard.as_ref() {
        Some(dict) => Ok(dict.set.contains(&word.to_lowercase())),
        None => Err("Spellchecker not initialized".into()),
    }
}

pub fn suggest_edit_n(word: &str, max: usize) -> Result<Vec<String>, String> {
    let guard = DICT.lock().map_err(|e| format!("Lock error: {e}"))?;
    let dict = match guard.as_ref() {
        Some(d) => d,
        None => return Err("Spellchecker not initialized".into()),
    };

    let word_lower = word.to_lowercase();
    let chars: Vec<char> = word_lower.chars().collect();
    let mut candidates: HashSet<String> = HashSet::new();

    // Deletions
    for i in 0..chars.len() {
        let mut c = chars.clone();
        c.remove(i);
        candidates.insert(c.into_iter().collect());
    }

    // Transpositions
    for i in 0..chars.len().saturating_sub(1) {
        let mut c = chars.clone();
        c.swap(i, i + 1);
        candidates.insert(c.into_iter().collect());
    }

    // Substitutions
    for i in 0..chars.len() {
        for &letter in LETTERS {
            let mut c = chars.clone();
            c[i] = letter;
            candidates.insert(c.into_iter().collect());
        }
    }

    // Insertions
    for i in 0..=chars.len() {
        for &letter in LETTERS {
            let mut c = chars.clone();
            c.insert(i, letter);
            candidates.insert(c.into_iter().collect());
        }
    }

    let mut found: Vec<String> = candidates
        .into_iter()
        .filter(|c| dict.set.contains(c))
        .collect();

    // Sort: common words first, then by length, then alphabetical
    found.sort_by(|a, b| {
        let a_common = dict.common.contains(a);
        let b_common = dict.common.contains(b);
        b_common
            .cmp(&a_common)
            .then_with(|| a.len().cmp(&b.len()))
            .then_with(|| a.cmp(b))
    });
    found.truncate(max);

    if !found.is_empty() {
        return Ok(found);
    }

    // Second pass: edit distance 2
    let mut edit1: Vec<String> = Vec::new();
    for i in 0..chars.len() {
        let mut base = chars.clone();
        base.remove(i);
        for j in 0..base.len() {
            for &letter in LETTERS {
                let mut c = base.clone();
                c[j] = letter;
                edit1.push(c.into_iter().collect());
            }
        }
    }

    let mut found2: Vec<String> = edit1
        .into_iter()
        .filter(|c| dict.set.contains(c))
        .collect();

    found2.sort_by(|a, b| {
        let a_common = dict.common.contains(a);
        let b_common = dict.common.contains(b);
        b_common
            .cmp(&a_common)
            .then_with(|| a.len().cmp(&b.len()))
            .then_with(|| a.cmp(b))
    });
    found2.truncate(max);

    Ok(found2)
}

pub fn suggest_prefix_n(prefix: &str, max: usize) -> Result<Vec<String>, String> {
    let guard = DICT.lock().map_err(|e| format!("Lock error: {e}"))?;
    let dict = match guard.as_ref() {
        Some(d) => d,
        None => return Err("Spellchecker not initialized".into()),
    };

    let prefix = prefix.to_lowercase();
    if prefix.is_empty() {
        return Ok(Vec::new());
    }

    // Binary search to skip irrelevant words (dict is sorted)
    let start = dict.sorted.partition_point(|w| w.as_str() < prefix.as_str());

    let mut common_matches: Vec<String> = Vec::with_capacity(4);
    let mut other_matches: Vec<String> = Vec::new();

    for word in &dict.sorted[start..] {
        if !word.starts_with(&prefix) {
            break;
        }
        if *word == prefix {
            continue;
        }
        if dict.common.contains(word) {
            common_matches.push(word.clone());
        } else {
            other_matches.push(word.clone());
        }
    }

    // Sort remaining by length (shorter = more common in practice)
    other_matches.sort_unstable_by_key(|w| w.len());

    let mut results = common_matches;
    results.extend(other_matches);
    results.truncate(max);
    Ok(results)
}

pub async fn download_dictionary(language: &str, url: &str) -> Result<(), String> {
    let dir = dict_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create dict dir: {e}"))?;

    // If URL is empty, auto-resolve from the language code
    let base_url = if url.is_empty() {
        dict_url(language)
    } else {
        url.to_string()
    };

    let aff_url = format!("{base_url}.aff");
    let dic_url = format!("{base_url}.dic");
    let aff_dest = aff_path(language);
    let dic_dest = dic_path(language);

    eprintln!("[plume] Downloading dictionary {language} from {base_url}...");
    download_file(&aff_url, &aff_dest).await?;
    download_file(&dic_url, &dic_dest).await?;
    eprintln!("[plume] Dictionary {language} ready");
    Ok(())
}

async fn download_file(url: &str, dest: &PathBuf) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Read failed: {e}"))?;

    std::fs::write(dest, &bytes).map_err(|e| format!("Write failed: {e}"))?;
    Ok(())
}
