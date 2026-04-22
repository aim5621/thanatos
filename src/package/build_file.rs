use reqwest::blocking::get;

pub fn fetch_build_file(package_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
        package_name
    );

    let body = get(&url)?.text()?;
    Ok(body)
}

pub struct PkgBuild {
    pub name: String,
    pub version: String,
    pub release: u32,
    pub depends: Vec<String>,
    pub make_depends: Vec<String>,
    pub sources: Vec<String>,
    pub checksums: Vec<String>,
    pub build_fn: String,
    pub package_fn: String,
}

pub fn parse_pkgbuild(content: &str) -> Result<PkgBuild, Box<dyn std::error::Error>> {
    let mut name = String::new();
    let mut version = String::new();
    let mut release = 1u32;
    let mut depends = vec![];
    let mut make_depends = vec![];
    let mut sources = vec![];
    let mut checksums = vec![];
    let mut build_fn = String::new();
    let mut package_fn = String::new();

    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();

        if line.starts_with("pkgname=") {
            name = line
                .trim_start_matches("pkgname=")
                .trim_matches('\'')
                .to_string();
        } else if line.starts_with("pkgver=") {
            version = line.trim_start_matches("pkgver=").to_string();
        } else if line.starts_with("pkgrel=") {
            release = line.trim_start_matches("pkgrel=").parse()?;
        } else if line.starts_with("depends=") {
            depends = parse_array(line, &mut lines);
        } else if line.starts_with("makedepends=") {
            make_depends = parse_array(line, &mut lines);
        } else if line.starts_with("source=") {
            sources = parse_array(line, &mut lines);
        } else if line.starts_with("sha256sums=") {
            checksums = parse_array(line, &mut lines);
        } else if line.starts_with("build()") {
            build_fn = parse_function(&mut lines);
        } else if line.starts_with("package()") {
            package_fn = parse_function(&mut lines);
        }
    }

    Ok(PkgBuild {
        name,
        version,
        release,
        depends,
        make_depends,
        sources,
        checksums,
        build_fn,
        package_fn,
    })
}

fn parse_array(line: &str, lines: &mut std::iter::Peekable<std::str::Lines>) -> Vec<String> {
    let mut result = vec![];
    let mut buf = line.to_string();

    while !buf.contains(')') {
        if let Some(next) = lines.next() {
            buf.push_str(next);
        } else {
            break;
        }
    }

    let inner = buf
        .split('(')
        .nth(1)
        .unwrap_or("")
        .split(')')
        .next()
        .unwrap_or("");
    for item in inner.split_whitespace() {
        let clean = item.trim_matches('\'').trim_matches('"').to_string();
        if !clean.is_empty() {
            result.push(clean);
        }
    }
    result
}

fn parse_function(lines: &mut std::iter::Peekable<std::str::Lines>) -> String {
    let mut body = String::new();
    let mut depth = 0;

    for line in lines.by_ref() {
        if line.contains('{') {
            depth += 1;
        }
        if line.contains('}') {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        body.push_str(line);
        body.push('\n');
    }
    body
}
