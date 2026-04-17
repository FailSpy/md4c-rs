//! LaTeX-to-Unicode conversion for terminal rendering.
//!
//! Converts LaTeX math notation to Unicode approximations for display
//! in terminal UIs where full LaTeX rendering isn't possible.
//!
//! This is a rendering-only transformation — source text is never modified.

use std::borrow::Cow;

/// Convert a LaTeX math string to its Unicode approximation.
///
/// Fast path: returns `Cow::Borrowed` when no LaTeX commands are present.
///
/// Handles simple commands (`\alpha` → `α`), superscripts (`x^2` → `x²`),
/// subscripts (`x_i` → `xᵢ`), fractions, square roots, text wrappers,
/// sizing hints, and operator names.
pub fn latex_to_unicode(input: &str) -> Cow<'_, str> {
    // Fast path: no LaTeX markers → zero allocation
    if !input.bytes().any(|b| b == b'\\' || b == b'^' || b == b'_') {
        return Cow::Borrowed(input);
    }
    Cow::Owned(convert(input))
}

/// Single-pass O(n) converter.
fn convert(input: &str) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'\\' => {
                i += 1;
                if i >= len {
                    out.push('\\');
                    break;
                }
                // Single-char escapes
                match bytes[i] {
                    b'\\' => {
                        out.push('\\');
                        i += 1;
                    }
                    b'{' => {
                        out.push('{');
                        i += 1;
                    }
                    b'}' => {
                        out.push('}');
                        i += 1;
                    }
                    b',' => {
                        out.push('\u{2009}');
                        i += 1;
                    } // thin space
                    b';' => {
                        out.push(' ');
                        i += 1;
                    }
                    b':' => {
                        out.push('\u{2005}');
                        i += 1;
                    } // medium math space
                    b'!' => {
                        i += 1;
                    } // negative thin space → nothing
                    b' ' => {
                        out.push(' ');
                        i += 1;
                    }
                    b'|' => {
                        out.push('‖');
                        i += 1;
                    }
                    b'%' => {
                        out.push('%');
                        i += 1;
                    }
                    _ if bytes[i].is_ascii_alphabetic() => {
                        let cmd_start = i;
                        while i < len && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                        let cmd = &input[cmd_start..i];
                        // Skip optional trailing space after command
                        if i < len && bytes[i] == b' ' {
                            // Only consume space if next char isn't special
                            // (LaTeX eats spaces after command names)
                        }
                        i = handle_command(cmd, bytes, i, &mut out);
                    }
                    _ => {
                        // Unknown escape — emit the char
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            b'^' => {
                i += 1;
                i = handle_superscript(bytes, i, input, &mut out);
            }
            b'_' => {
                i += 1;
                i = handle_subscript(bytes, i, input, &mut out);
            }
            _ => {
                // Push the char (handles multi-byte UTF-8)
                let ch = &input[i..];
                if let Some(c) = ch.chars().next() {
                    out.push(c);
                    i += c.len_utf8();
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}

/// Handle a `\command` — returns the new position in `bytes`.
fn handle_command(cmd: &str, bytes: &[u8], mut pos: usize, out: &mut String) -> usize {
    // Structural commands that take braced arguments
    match cmd {
        "frac" => {
            if let Some((num, after_num)) = extract_braced(bytes, pos) {
                if let Some((den, after_den)) = extract_braced(bytes, after_num) {
                    let num_converted = convert(num);
                    let den_converted = convert(den);
                    out.push_str(&num_converted);
                    out.push('/');
                    out.push_str(&den_converted);
                    return after_den;
                }
            }
            out.push_str("frac");
            return pos;
        }
        "sqrt" => {
            // \sqrt[n]{x} or \sqrt{x}
            if pos < bytes.len() && bytes[pos] == b'[' {
                if let Some((degree, after_bracket)) = extract_bracketed(bytes, pos) {
                    let degree_converted = convert(degree);
                    // Try to superscript the degree
                    for c in degree_converted.chars() {
                        if let Some(sup) = to_superscript(c) {
                            out.push(sup);
                        } else {
                            out.push(c);
                        }
                    }
                    out.push('√');
                    if let Some((content, after_brace)) = extract_braced(bytes, after_bracket) {
                        let content_converted = convert(content);
                        out.push_str(&content_converted);
                        return after_brace;
                    }
                    return after_bracket;
                }
            }
            out.push('√');
            if let Some((content, after)) = extract_braced(bytes, pos) {
                let content_converted = convert(content);
                out.push_str(&content_converted);
                return after;
            }
            return pos;
        }
        "text" | "mathrm" | "textrm" | "textit" | "textbf" | "textsf" | "texttt" | "mathit"
        | "mathsf" | "mathtt" | "operatorname" => {
            if let Some((content, after)) = extract_braced(bytes, pos) {
                out.push_str(content);
                return after;
            }
            out.push_str(cmd);
            return pos;
        }
        "mathbf" | "boldsymbol" | "bm" => {
            if let Some((content, after)) = extract_braced(bytes, pos) {
                let converted = convert(content);
                out.push_str(&converted);
                return after;
            }
            out.push_str(cmd);
            return pos;
        }
        "mathbb" => {
            if let Some((content, after)) = extract_braced(bytes, pos) {
                for c in content.chars() {
                    out.push(blackboard_bold(c));
                }
                return after;
            }
            out.push_str("mathbb");
            return pos;
        }
        "mathcal" => {
            if let Some((content, after)) = extract_braced(bytes, pos) {
                for c in content.chars() {
                    out.push(math_calligraphic(c));
                }
                return after;
            }
            out.push_str("mathcal");
            return pos;
        }
        "mathfrak" => {
            if let Some((content, after)) = extract_braced(bytes, pos) {
                for c in content.chars() {
                    out.push(math_fraktur(c));
                }
                return after;
            }
            out.push_str("mathfrak");
            return pos;
        }
        // Accent/decoration commands: \hat{x} → x̂ etc.
        "hat" => return accent_command(bytes, pos, out, '\u{0302}', cmd),
        "bar" | "overline" => return accent_command(bytes, pos, out, '\u{0304}', cmd),
        "tilde" => return accent_command(bytes, pos, out, '\u{0303}', cmd),
        "vec" => return accent_command(bytes, pos, out, '\u{20D7}', cmd),
        "dot" => return accent_command(bytes, pos, out, '\u{0307}', cmd),
        "ddot" => return accent_command(bytes, pos, out, '\u{0308}', cmd),
        "check" => return accent_command(bytes, pos, out, '\u{030C}', cmd),
        "breve" => return accent_command(bytes, pos, out, '\u{0306}', cmd),
        "acute" => return accent_command(bytes, pos, out, '\u{0301}', cmd),
        "grave" => return accent_command(bytes, pos, out, '\u{0300}', cmd),
        // Sizing/delimiter commands — strip and emit delimiter
        "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr"
        | "biggl" | "biggr" | "Biggl" | "Biggr" => {
            // Next char is the delimiter
            if pos < bytes.len() {
                let delim = match bytes[pos] {
                    b'(' => '(',
                    b')' => ')',
                    b'[' => '[',
                    b']' => ']',
                    b'|' => '|',
                    b'.' => {
                        pos += 1;
                        return pos;
                    } // \left. = invisible delimiter
                    b'\\' => {
                        // e.g. \left\{ or \left\langle
                        pos += 1;
                        if pos < bytes.len() {
                            if bytes[pos] == b'{' {
                                out.push('{');
                                return pos + 1;
                            } else if bytes[pos] == b'}' {
                                out.push('}');
                                return pos + 1;
                            } else if bytes[pos] == b'|' {
                                out.push('‖');
                                return pos + 1;
                            }
                            // Read command name after backslash
                            let cs = pos;
                            while pos < bytes.len() && bytes[pos].is_ascii_alphabetic() {
                                pos += 1;
                            }
                            if cs < pos {
                                let inner_cmd = std::str::from_utf8(&bytes[cs..pos]).unwrap_or("");
                                if let Some(s) = lookup_command(inner_cmd) {
                                    out.push_str(s);
                                }
                            }
                        }
                        return pos;
                    }
                    _ => {
                        return pos;
                    }
                };
                out.push(delim);
                return pos + 1;
            }
            return pos;
        }
        _ => {}
    }

    // Simple command lookup
    if let Some(s) = lookup_command(cmd) {
        out.push_str(s);
        return pos;
    }

    // Operator names: \log, \sin, etc. → strip backslash
    if is_operator_name(cmd) {
        out.push_str(cmd);
        return pos;
    }

    // Unknown command: strip backslash, emit name
    out.push_str(cmd);
    pos
}

fn accent_command(bytes: &[u8], pos: usize, out: &mut String, combining: char, cmd: &str) -> usize {
    if let Some((content, after)) = extract_braced(bytes, pos) {
        let converted = convert(content);
        for (i, c) in converted.chars().enumerate() {
            out.push(c);
            if i == 0 {
                out.push(combining);
            }
        }
        return after;
    }
    // No braces — apply to next single char
    if pos < bytes.len() && bytes[pos] != b'\\' && bytes[pos] != b'{' {
        if let Some(c) = std::str::from_utf8(&bytes[pos..])
            .ok()
            .and_then(|s| s.chars().next())
        {
            out.push(c);
            out.push(combining);
            return pos + c.len_utf8();
        }
    }
    out.push_str(cmd);
    pos
}

/// Handle `^` — superscript.
fn handle_superscript(bytes: &[u8], pos: usize, input: &str, out: &mut String) -> usize {
    if pos >= bytes.len() {
        out.push('^');
        return pos;
    }
    if bytes[pos] == b'{' {
        if let Some((content, after)) = extract_braced(bytes, pos) {
            let converted = convert(content);
            let mut any_failed = false;
            let mut sup_buf = String::new();
            for c in converted.chars() {
                if let Some(sup) = to_superscript(c) {
                    sup_buf.push(sup);
                } else {
                    any_failed = true;
                    break;
                }
            }
            if any_failed {
                out.push('^');
                out.push('(');
                out.push_str(&converted);
                out.push(')');
            } else {
                out.push_str(&sup_buf);
            }
            return after;
        }
        out.push('^');
        return pos;
    }
    // Single char superscript
    let s = &input[pos..];
    if let Some(c) = s.chars().next() {
        if let Some(sup) = to_superscript(c) {
            out.push(sup);
        } else {
            out.push('^');
            out.push(c);
        }
        return pos + c.len_utf8();
    }
    out.push('^');
    pos
}

/// Handle `_` — subscript.
fn handle_subscript(bytes: &[u8], pos: usize, input: &str, out: &mut String) -> usize {
    if pos >= bytes.len() {
        out.push('_');
        return pos;
    }
    if bytes[pos] == b'{' {
        if let Some((content, after)) = extract_braced(bytes, pos) {
            let converted = convert(content);
            let mut any_failed = false;
            let mut sub_buf = String::new();
            for c in converted.chars() {
                if let Some(sub) = to_subscript(c) {
                    sub_buf.push(sub);
                } else {
                    any_failed = true;
                    break;
                }
            }
            if any_failed {
                out.push('_');
                out.push('(');
                out.push_str(&converted);
                out.push(')');
            } else {
                out.push_str(&sub_buf);
            }
            return after;
        }
        out.push('_');
        return pos;
    }
    // Single char subscript
    let s = &input[pos..];
    if let Some(c) = s.chars().next() {
        if let Some(sub) = to_subscript(c) {
            out.push(sub);
        } else {
            out.push('_');
            out.push(c);
        }
        return pos + c.len_utf8();
    }
    out.push('_');
    pos
}

/// Extract content from `{...}` handling nested braces. Returns `(content, pos_after_close)`.
fn extract_braced(bytes: &[u8], pos: usize) -> Option<(&str, usize)> {
    if pos >= bytes.len() || bytes[pos] != b'{' {
        return None;
    }
    let mut depth = 1;
    let mut i = pos + 1;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\\' => {
                i += 1;
            } // skip escaped char
            _ => {}
        }
        if depth > 0 {
            i += 1;
        }
    }
    if depth == 0 {
        let content = std::str::from_utf8(&bytes[pos + 1..i]).ok()?;
        Some((content, i + 1))
    } else {
        None
    }
}

/// Extract content from `[...]`. Returns `(content, pos_after_close)`.
fn extract_bracketed(bytes: &[u8], pos: usize) -> Option<(&str, usize)> {
    if pos >= bytes.len() || bytes[pos] != b'[' {
        return None;
    }
    let mut depth = 1;
    let mut i = pos + 1;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => depth -= 1,
            b'\\' => {
                i += 1;
            }
            _ => {}
        }
        if depth > 0 {
            i += 1;
        }
    }
    if depth == 0 {
        let content = std::str::from_utf8(&bytes[pos + 1..i]).ok()?;
        Some((content, i + 1))
    } else {
        None
    }
}

fn to_superscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' | '−' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        'A' => 'ᴬ',
        'B' => 'ᴮ',
        'D' => 'ᴰ',
        'E' => 'ᴱ',
        'G' => 'ᴳ',
        'H' => 'ᴴ',
        'I' => 'ᴵ',
        'J' => 'ᴶ',
        'K' => 'ᴷ',
        'L' => 'ᴸ',
        'M' => 'ᴹ',
        'N' => 'ᴺ',
        'O' => 'ᴼ',
        'P' => 'ᴾ',
        'R' => 'ᴿ',
        'T' => 'ᵀ',
        'U' => 'ᵁ',
        'V' => 'ⱽ',
        'W' => 'ᵂ',
        '*' => '˟',
        ' ' => ' ',
        _ => return None,
    })
}

fn to_subscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '−' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        ' ' => ' ',
        _ => return None,
    })
}

fn blackboard_bold(c: char) -> char {
    match c {
        'A' => '𝔸',
        'B' => '𝔹',
        'C' => 'ℂ',
        'D' => '𝔻',
        'E' => '𝔼',
        'F' => '𝔽',
        'G' => '𝔾',
        'H' => 'ℍ',
        'I' => '𝕀',
        'J' => '𝕁',
        'K' => '𝕂',
        'L' => '𝕃',
        'M' => '𝕄',
        'N' => 'ℕ',
        'O' => '𝕆',
        'P' => 'ℙ',
        'Q' => 'ℚ',
        'R' => 'ℝ',
        'S' => '𝕊',
        'T' => '𝕋',
        'U' => '𝕌',
        'V' => '𝕍',
        'W' => '𝕎',
        'X' => '𝕏',
        'Y' => '𝕐',
        'Z' => 'ℤ',
        '0' => '𝟘',
        '1' => '𝟙',
        '2' => '𝟚',
        '3' => '𝟛',
        '4' => '𝟜',
        '5' => '𝟝',
        '6' => '𝟞',
        '7' => '𝟟',
        '8' => '𝟠',
        '9' => '𝟡',
        _ => c,
    }
}

fn math_calligraphic(c: char) -> char {
    match c {
        'A' => '𝒜',
        'B' => 'ℬ',
        'C' => '𝒞',
        'D' => '𝒟',
        'E' => 'ℰ',
        'F' => 'ℱ',
        'G' => '𝒢',
        'H' => 'ℋ',
        'I' => 'ℐ',
        'J' => '𝒥',
        'K' => '𝒦',
        'L' => 'ℒ',
        'M' => 'ℳ',
        'N' => '𝒩',
        'O' => '𝒪',
        'P' => '𝒫',
        'Q' => '𝒬',
        'R' => 'ℛ',
        'S' => '𝒮',
        'T' => '𝒯',
        'U' => '𝒰',
        'V' => '𝒱',
        'W' => '𝒲',
        'X' => '𝒳',
        'Y' => '𝒴',
        'Z' => '𝒵',
        _ => c,
    }
}

fn math_fraktur(c: char) -> char {
    match c {
        'A' => '𝔄',
        'B' => '𝔅',
        'C' => 'ℭ',
        'D' => '𝔇',
        'E' => '𝔈',
        'F' => '𝔉',
        'G' => '𝔊',
        'H' => 'ℌ',
        'I' => 'ℑ',
        'J' => '𝔍',
        'K' => '𝔎',
        'L' => '𝔏',
        'M' => '𝔐',
        'N' => '𝔑',
        'O' => '𝔒',
        'P' => '𝔓',
        'Q' => '𝔔',
        'R' => 'ℜ',
        'S' => '𝔖',
        'T' => '𝔗',
        'U' => '𝔘',
        'V' => '𝔙',
        'W' => '𝔚',
        'X' => '𝔛',
        'Y' => '𝔜',
        'Z' => 'ℨ',
        'a' => '𝔞',
        'b' => '𝔟',
        'c' => '𝔠',
        'd' => '𝔡',
        'e' => '𝔢',
        'f' => '𝔣',
        'g' => '𝔤',
        'h' => '𝔥',
        'i' => '𝔦',
        'j' => '𝔧',
        'k' => '𝔨',
        'l' => '𝔩',
        'm' => '𝔪',
        'n' => '𝔫',
        'o' => '𝔬',
        'p' => '𝔭',
        'q' => '𝔮',
        'r' => '𝔯',
        's' => '𝔰',
        't' => '𝔱',
        'u' => '𝔲',
        'v' => '𝔳',
        'w' => '𝔴',
        'x' => '𝔵',
        'y' => '𝔶',
        'z' => '𝔷',
        _ => c,
    }
}

fn is_operator_name(cmd: &str) -> bool {
    matches!(
        cmd,
        "log"
            | "ln"
            | "exp"
            | "sin"
            | "cos"
            | "tan"
            | "cot"
            | "sec"
            | "csc"
            | "arcsin"
            | "arccos"
            | "arctan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "lim"
            | "limsup"
            | "liminf"
            | "sup"
            | "inf"
            | "min"
            | "max"
            | "arg"
            | "det"
            | "dim"
            | "ker"
            | "hom"
            | "deg"
            | "gcd"
            | "Pr"
            | "mod"
            | "bmod"
    )
}

/// Lookup table for simple `\command` → Unicode mappings.
fn lookup_command(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        // Greek lowercase
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "φ",
        "varphi" => "ϕ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        // Greek uppercase
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",

        // Arrows
        "to" | "rightarrow" => "→",
        "leftarrow" | "gets" => "←",
        "leftrightarrow" => "↔",
        "Rightarrow" => "⇒",
        "Leftarrow" => "⇐",
        "Leftrightarrow" => "⇔",
        "implies" => "⟹",
        "iff" => "⟺",
        "impliedby" => "⟸",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "updownarrow" => "↕",
        "Uparrow" => "⇑",
        "Downarrow" => "⇓",
        "mapsto" => "↦",
        "longmapsto" => "⟼",
        "nearrow" => "↗",
        "searrow" => "↘",
        "swarrow" => "↙",
        "nwarrow" => "↖",
        "hookrightarrow" => "↪",
        "hookleftarrow" => "↩",
        "longrightarrow" => "⟶",
        "longleftarrow" => "⟵",
        "longleftrightarrow" => "⟷",
        "Longrightarrow" => "⟹",
        "Longleftarrow" => "⟸",
        "Longleftrightarrow" => "⟺",
        "rightrightarrows" => "⇉",
        "leftleftarrows" => "⇇",
        "rightleftharpoons" => "⇌",
        "leftrightharpoons" => "⇋",

        // Binary operators
        "times" => "×",
        "div" => "÷",
        "pm" => "±",
        "mp" => "∓",
        "cdot" => "·",
        "circ" => "∘",
        "ast" => "∗",
        "star" => "⋆",
        "bullet" => "∙",
        "oplus" => "⊕",
        "ominus" => "⊖",
        "otimes" => "⊗",
        "odot" => "⊙",
        "oslash" => "⊘",
        "dagger" => "†",
        "ddagger" => "‡",
        "amalg" => "⨿",
        "wr" => "≀",

        // Relations
        "leq" | "le" => "≤",
        "geq" | "ge" => "≥",
        "neq" | "ne" => "≠",
        "approx" => "≈",
        "sim" => "∼",
        "simeq" => "≃",
        "cong" => "≅",
        "equiv" => "≡",
        "propto" => "∝",
        "prec" => "≺",
        "succ" => "≻",
        "preceq" => "⪯",
        "succeq" => "⪰",
        "ll" => "≪",
        "gg" => "≫",
        "subset" => "⊂",
        "supset" => "⊃",
        "subseteq" => "⊆",
        "supseteq" => "⊇",
        "sqsubseteq" => "⊑",
        "sqsupseteq" => "⊒",
        "in" => "∈",
        "ni" | "owns" => "∋",
        "notin" => "∉",
        "vdash" => "⊢",
        "dashv" => "⊣",
        "models" => "⊨",
        "mid" => "∣",
        "nmid" => "∤",
        "parallel" => "∥",
        "nparallel" => "∦",
        "perp" => "⊥",
        "asymp" => "≍",
        "doteq" => "≐",
        "triangleleft" => "◁",
        "triangleright" => "▷",

        // Logic
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "neg" | "lnot" => "¬",
        "land" | "wedge" => "∧",
        "lor" | "vee" => "∨",
        "top" => "⊤",
        "bot" => "⊥",
        "therefore" => "∴",
        "because" => "∵",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "iddots" => "⋰",

        // Set theory
        "emptyset" | "varnothing" | "empty" => "∅",
        "cap" => "∩",
        "cup" => "∪",
        "setminus" => "∖",
        "complement" => "∁",
        "bigcap" => "⋂",
        "bigcup" => "⋃",
        "bigsqcup" => "⨆",
        "sqcap" => "⊓",
        "sqcup" => "⊔",

        // Big operators
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "oiint" => "∯",
        "bigwedge" => "⋀",
        "bigvee" => "⋁",
        "bigoplus" => "⨁",
        "bigotimes" => "⨂",
        "bigodot" => "⨀",

        // Misc symbols
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "hbar" => "ℏ",
        "ell" => "ℓ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        "beth" => "ℶ",
        "gimel" => "ℷ",
        "wp" => "℘",
        "angle" => "∠",
        "measuredangle" => "∡",
        "triangle" => "△",
        "square" => "□",
        "Diamond" => "◇",
        "lozenge" => "◊",
        "clubsuit" => "♣",
        "diamondsuit" => "♢",
        "heartsuit" => "♡",
        "spadesuit" => "♠",
        "flat" => "♭",
        "natural" => "♮",
        "sharp" => "♯",
        "surd" => "√",
        "prime" => "′",
        "backprime" => "‵",
        "ldots" | "dots" | "dotsc" => "…",
        "cdots" | "dotsb" | "dotsi" | "dotsm" => "⋯",
        "S" => "§",
        "P" => "¶",
        "checkmark" => "✓",

        // Delimiters
        "langle" => "⟨",
        "rangle" => "⟩",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lbrace" => "{",
        "rbrace" => "}",
        "lbrack" => "[",
        "rbrack" => "]",
        "vert" => "|",
        "Vert" => "‖",
        "lVert" => "‖",
        "rVert" => "‖",
        "lvert" => "|",
        "rvert" => "|",
        "backslash" => "\\",

        // Spacing
        "quad" => "  ",
        "qquad" => "    ",
        "enspace" => " ",
        "thinspace" => "\u{2009}",

        // Misc text
        "LaTeX" => "LaTeX",
        "TeX" => "TeX",
        "amp" => "&",
        "colon" => ":",

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_path_no_alloc() {
        let result = latex_to_unicode("plain text");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "plain text");
    }

    #[test]
    fn empty_input() {
        assert_eq!(latex_to_unicode(""), "");
    }

    #[test]
    fn simple_commands() {
        assert_eq!(latex_to_unicode("\\alpha + \\beta"), "α + β");
        assert_eq!(latex_to_unicode("\\rightarrow"), "→");
        assert_eq!(latex_to_unicode("\\to"), "→");
        assert_eq!(latex_to_unicode("\\infty"), "∞");
        assert_eq!(latex_to_unicode("\\neq"), "≠");
        assert_eq!(latex_to_unicode("\\leq"), "≤");
        assert_eq!(latex_to_unicode("\\geq"), "≥");
    }

    #[test]
    fn greek_letters() {
        assert_eq!(latex_to_unicode("\\Gamma \\Delta \\Theta"), "Γ Δ Θ");
        assert_eq!(latex_to_unicode("\\pi r^2"), "π r²");
    }

    #[test]
    fn arrows() {
        assert_eq!(latex_to_unicode("A \\Rightarrow B"), "A ⇒ B");
        assert_eq!(latex_to_unicode("f: X \\to Y"), "f: X → Y");
        assert_eq!(latex_to_unicode("\\Leftrightarrow"), "⇔");
    }

    #[test]
    fn superscripts() {
        assert_eq!(latex_to_unicode("x^2"), "x²");
        assert_eq!(latex_to_unicode("x^{10}"), "x¹⁰");
        assert_eq!(latex_to_unicode("x^n"), "xⁿ");
        assert_eq!(latex_to_unicode("e^{i\\pi}"), "e^(iπ)"); // π has no superscript form
        assert_eq!(latex_to_unicode("x^{2n+1}"), "x²ⁿ⁺¹");
    }

    #[test]
    fn subscripts() {
        assert_eq!(latex_to_unicode("x_i"), "xᵢ");
        assert_eq!(latex_to_unicode("a_{12}"), "a₁₂");
        assert_eq!(latex_to_unicode("x_{n+1}"), "xₙ₊₁");
    }

    #[test]
    fn superscript_fallback() {
        // 'q' has no Unicode superscript
        assert_eq!(latex_to_unicode("x^{q}"), "x^(q)");
    }

    #[test]
    fn subscript_fallback() {
        // 'z' has no Unicode subscript
        assert_eq!(latex_to_unicode("x_{z}"), "x_(z)");
    }

    #[test]
    fn fractions() {
        assert_eq!(latex_to_unicode("\\frac{1}{2}"), "1/2");
        assert_eq!(latex_to_unicode("\\frac{a+b}{c+d}"), "a+b/c+d");
        assert_eq!(latex_to_unicode("\\frac{\\alpha}{\\beta}"), "α/β");
    }

    #[test]
    fn sqrt() {
        assert_eq!(latex_to_unicode("\\sqrt{x}"), "√x");
        assert_eq!(latex_to_unicode("\\sqrt{x+1}"), "√x+1");
    }

    #[test]
    fn text_wrappers() {
        assert_eq!(latex_to_unicode("\\text{hello}"), "hello");
        assert_eq!(latex_to_unicode("\\mathrm{Var}"), "Var");
        assert_eq!(latex_to_unicode("\\mathbf{x}"), "x");
    }

    #[test]
    fn blackboard_bold() {
        assert_eq!(latex_to_unicode("\\mathbb{R}"), "ℝ");
        assert_eq!(latex_to_unicode("\\mathbb{N}"), "ℕ");
        assert_eq!(latex_to_unicode("\\mathbb{Z}"), "ℤ");
        assert_eq!(latex_to_unicode("\\mathbb{C}"), "ℂ");
        assert_eq!(latex_to_unicode("\\mathbb{Q}"), "ℚ");
    }

    #[test]
    fn calligraphic() {
        assert_eq!(latex_to_unicode("\\mathcal{L}"), "ℒ");
        assert_eq!(latex_to_unicode("\\mathcal{F}"), "ℱ");
    }

    #[test]
    fn fraktur() {
        assert_eq!(latex_to_unicode("\\mathfrak{g}"), "𝔤");
    }

    #[test]
    fn sizing_stripping() {
        assert_eq!(latex_to_unicode("\\left(x\\right)"), "(x)");
        assert_eq!(latex_to_unicode("\\big(x\\big)"), "(x)");
    }

    #[test]
    fn operator_names() {
        assert_eq!(latex_to_unicode("\\log x"), "log x");
        assert_eq!(latex_to_unicode("\\sin \\theta"), "sin θ");
        assert_eq!(latex_to_unicode("\\lim"), "lim");
    }

    #[test]
    fn delimiters() {
        assert_eq!(latex_to_unicode("\\langle x \\rangle"), "⟨ x ⟩");
        assert_eq!(latex_to_unicode("\\lfloor x \\rfloor"), "⌊ x ⌋");
    }

    #[test]
    fn accents() {
        assert_eq!(latex_to_unicode("\\hat{x}"), "x\u{0302}");
        assert_eq!(latex_to_unicode("\\bar{x}"), "x\u{0304}");
        assert_eq!(latex_to_unicode("\\vec{v}"), "v\u{20D7}");
        assert_eq!(latex_to_unicode("\\tilde{x}"), "x\u{0303}");
    }

    #[test]
    fn mixed_complex() {
        assert_eq!(latex_to_unicode("\\alpha^{2} + \\beta_{i}"), "α² + βᵢ");
        assert_eq!(latex_to_unicode("\\sum_{i=0}^{n} x_i"), "∑ᵢ₌₀ⁿ xᵢ");
        assert_eq!(latex_to_unicode("\\int_0^1 f(x) dx"), "∫₀¹ f(x) dx");
    }

    #[test]
    fn unknown_command() {
        assert_eq!(latex_to_unicode("\\customop"), "customop");
    }

    #[test]
    fn escaped_braces() {
        assert_eq!(latex_to_unicode("\\{x\\}"), "{x}");
    }

    #[test]
    fn spacing_commands() {
        assert_eq!(latex_to_unicode("a\\quad b"), "a   b"); // \quad = 2 spaces + literal space
        assert_eq!(latex_to_unicode("a\\qquad b"), "a     b"); // \qquad = 4 spaces + literal space
    }

    #[test]
    fn logic_symbols() {
        assert_eq!(latex_to_unicode("\\forall x \\exists y"), "∀ x ∃ y");
        assert_eq!(latex_to_unicode("A \\land B \\lor C"), "A ∧ B ∨ C");
    }

    #[test]
    fn set_theory() {
        assert_eq!(latex_to_unicode("A \\cup B \\cap C"), "A ∪ B ∩ C");
        assert_eq!(latex_to_unicode("x \\in \\emptyset"), "x ∈ ∅");
        assert_eq!(latex_to_unicode("A \\setminus B"), "A ∖ B");
    }

    #[test]
    fn big_operators() {
        assert_eq!(latex_to_unicode("\\sum \\prod \\int"), "∑ ∏ ∫");
        assert_eq!(latex_to_unicode("\\oint \\iint"), "∮ ∬");
    }

    #[test]
    fn dots() {
        assert_eq!(latex_to_unicode("a_1, \\ldots, a_n"), "a₁, …, aₙ");
        assert_eq!(latex_to_unicode("a_1 + \\cdots + a_n"), "a₁ + ⋯ + aₙ");
    }

    #[test]
    fn left_right_with_backslash_delim() {
        assert_eq!(latex_to_unicode("\\left\\{x\\right\\}"), "{x}");
    }

    #[test]
    fn nested_frac() {
        assert_eq!(latex_to_unicode("\\frac{\\frac{a}{b}}{c}"), "a/b/c");
    }

    #[test]
    fn sqrt_with_degree() {
        assert_eq!(latex_to_unicode("\\sqrt[3]{x}"), "³√x");
    }

    #[test]
    fn trailing_backslash() {
        assert_eq!(latex_to_unicode("test\\"), "test\\");
    }

    #[test]
    fn preserves_plain_text() {
        assert_eq!(latex_to_unicode("f(x) = 2x + 1"), "f(x) = 2x + 1");
    }
}
