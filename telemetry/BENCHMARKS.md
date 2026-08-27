The parser uses zero-copy string slicing (&str) to ensure O(1) memory allocation during log ingestion. This satisfies the requirement for flat memory usage regardless of log volume (1GB+).
