
#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::storage::MockStorage;
    use crate::services::ocr::MockOcr;

    // You'd need a MockRepo or use sqlx::test, but for unit testing logic we can test private methods
    // or refactor to allow mocking repo easily (repo is concrete struct, needs trait for full mocking).
    // But `normalize_name` is easy to test.

    #[test]
    fn test_normalize_name() {
        let service = KycService {
            repo: unsafe { std::mem::zeroed() }, // Hack: Don't use this in real test running methods that use repo
            storage: Arc::new(MockStorage),
            ocr: Arc::new(MockOcr { expected_text: "".to_string() }),
        };

        assert_eq!(service.normalize_name("Shubham Dixit"), "SHUBHAM DIXIT");
        assert_eq!(service.normalize_name("  Shubham   Dixit  "), "SHUBHAM DIXIT");
        assert_eq!(service.normalize_name("Shubham.Dixit"), "SHUBHAM DIXIT");
        assert_eq!(service.normalize_name("Shubham-Dixit"), "SHUBHAM DIXIT");
    }

    // Checking the matching logic extractable?
    // It's inside `submit_kyc`, which uses repo.
    // Ideally `extract_name_from_text` should be tested too.
    #[test]
    fn test_extract_name_heuristic() {
         let service = KycService {
            repo: unsafe { std::mem::zeroed() },
            storage: Arc::new(MockStorage),
            ocr: Arc::new(MockOcr { expected_text: "".to_string() }),
        };

        let text = "GOVERNMENT OF INDIA\nShubham Dixit\nDOB: ...";
        let extracted = service.extract_name_from_text(text, &KycDocType::Aadhaar);
        // My heuristic in `extract_name_from_text` simple joins lines > 3 chars.
        // "GOVERNMENT OF INDIA Shubham Dixit DOB: ..."
        assert!(extracted.contains("SHUBHAM DIXIT")); // Normalized comparison would leverage this
    }
}
