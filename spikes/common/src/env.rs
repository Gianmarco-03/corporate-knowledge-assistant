//! Lettura di variabili d'ambiente obbligatorie senza mai esporre il valore.

pub fn required(_name: &str) -> anyhow::Result<String> {
    todo!()
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
        assert!(msg.contains(name), "il messaggio deve nominare la variabile: {msg}");
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
            assert!(msg.contains(name), "il messaggio deve nominare la variabile: {msg}");
            assert!(!msg.contains(sentinel), "il valore non deve comparire: {msg}");
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
        assert!(msg.contains(name), "il messaggio deve nominare la variabile: {msg}");
        let dbg = format!("{err:?}");
        assert!(!dbg.contains("\"   \""), "il valore non deve comparire: {dbg}");
    }
}
