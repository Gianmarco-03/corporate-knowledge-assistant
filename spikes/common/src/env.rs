//! Lettura di variabili d'ambiente obbligatorie senza mai esporre il valore.
//!
//! Ogni errore nomina la variabile e il motivo, mai il contenuto: la variabile
//! può essere un segreto (chiavi API, password del database). Per questo gli
//! errori della libreria standard (`VarError::NotUnicode` porta con sé il
//! valore grezzo) non vengono mai incatenati come causa.
use anyhow::{Context, bail};

/// Legge la variabile `name` e la restituisce solo se è impostata, è UTF-8
/// valido, non è vuota dopo il trim e non ha spazi in testa o in coda né a-capo
/// (tipico errore di copia in `.env`).
pub fn required(name: &str) -> anyhow::Result<String> {
    if name.is_empty() || name.contains(['=', '\0']) {
        bail!("nome di variabile d'ambiente non valido: {name:?}");
    }
    let raw = std::env::var_os(name)
        .with_context(|| format!("variabile d'ambiente {name} non impostata"))?;
    let Ok(value) = raw.into_string() else {
        bail!("variabile d'ambiente {name} non contiene UTF-8 valido");
    };
    if value.trim().is_empty() {
        bail!("variabile d'ambiente {name} impostata ma vuota");
    }
    if value.trim() != value {
        bail!("variabile d'ambiente {name} ha spazi o a-capo in testa o in coda");
    }
    if value.contains(['\n', '\r']) {
        bail!("variabile d'ambiente {name} contiene un a-capo");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_required_unset_names_variable() {
        let name = "CKA_SPIKE_TEST_UNSET_VAR";
        // SAFETY: nome univoco usato solo da questo test; nessun altro test lo legge o lo scrive.
        unsafe { std::env::remove_var(name) };
        let err = required(name).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains(name),
            "il messaggio deve nominare la variabile: {msg}"
        );
    }

    #[test]
    fn env_required_error_never_contains_value() {
        let name = "CKA_SPIKE_TEST_ENV_REQUIRED_ERROR_NEVER_CONTAINS_VALUE";
        let sentinel = "CKA_SPIKE_SENTINEL_7f3a9c";
        // SAFETY: nome univoco usato solo da questo test; nessun altro test lo legge o lo scrive.
        unsafe { std::env::set_var(name, format!("{sentinel}\n")) };
        let res = required(name);
        // SAFETY: stesso nome univoco, rimozione a fine test.
        unsafe { std::env::remove_var(name) };
        let err = res.unwrap_err();
        for msg in [format!("{err:#}"), format!("{err:?}")] {
            assert!(
                msg.contains(name),
                "il messaggio deve nominare la variabile: {msg}"
            );
            assert!(
                !msg.contains(sentinel),
                "il valore non deve comparire: {msg}"
            );
        }
    }

    #[test]
    fn env_required_blank_value_is_rejected_without_leaking() {
        let name = "CKA_SPIKE_TEST_ENV_REQUIRED_BLANK_VALUE_IS_REJECTED_WITHOUT_LEAKING";
        // SAFETY: nome univoco usato solo da questo test; nessun altro test lo legge o lo scrive.
        unsafe { std::env::set_var(name, "   ") };
        let res = required(name);
        // SAFETY: stesso nome univoco, rimozione a fine test.
        unsafe { std::env::remove_var(name) };
        let err = res.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains(name),
            "il messaggio deve nominare la variabile: {msg}"
        );
        let dbg = format!("{err:?}");
        assert!(
            !dbg.contains("\"   \""),
            "il valore non deve comparire: {dbg}"
        );
    }

    #[test]
    fn env_required_valid_value_is_returned() {
        let name = "CKA_SPIKE_TEST_ENV_REQUIRED_VALID_VALUE_IS_RETURNED";
        // SAFETY: nome univoco usato solo da questo test; nessun altro test lo legge o lo scrive.
        unsafe { std::env::set_var(name, "postgres://u@h/db") };
        let res = required(name);
        // SAFETY: stesso nome univoco, rimozione a fine test.
        unsafe { std::env::remove_var(name) };
        assert_eq!(res.unwrap(), "postgres://u@h/db");
    }

    #[test]
    fn env_required_inner_newline_is_rejected_without_leaking() {
        let name = "CKA_SPIKE_TEST_ENV_REQUIRED_INNER_NEWLINE_IS_REJECTED_WITHOUT_LEAKING";
        let sentinel = "CKA_SPIKE_SENTINEL_b81e04";
        // SAFETY: nome univoco usato solo da questo test; nessun altro test lo legge o lo scrive.
        unsafe { std::env::set_var(name, format!("{sentinel}\r\nsecond")) };
        let res = required(name);
        // SAFETY: stesso nome univoco, rimozione a fine test.
        unsafe { std::env::remove_var(name) };
        let err = res.unwrap_err();
        for msg in [format!("{err:#}"), format!("{err:?}")] {
            assert!(msg.contains(name), "{msg}");
            assert!(
                !msg.contains(sentinel),
                "il valore non deve comparire: {msg}"
            );
        }
    }
}
