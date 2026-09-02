pub struct SorobanMetrics<'a> {
    pub contract_id: &'a str,
    pub function_name: &'a str,
    pub cpu_instructions: u64, // Updated name
    pub memory_bytes: u64,      // Updated name
}

pub struct LogParser;

impl LogParser {
    pub fn parse_line(line: &str) -> Option<SorobanMetrics<'_>> {
        let contract_id = Self::extract(line, "contract_id=")?;
        let function_name = Self::extract(line, "function=")?;
        // Parsing the specific names from the prompt
        let cpu_instructions = Self::extract(line, "cpu_instructions=")?.parse().ok()?;
        let memory_bytes = Self::extract(line, "memory_bytes=")?.parse().ok()?;

        Some(SorobanMetrics { 
            contract_id, 
            function_name, 
            cpu_instructions, 
            memory_bytes 
        })
    }

    fn extract<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let start = line.find(key)? + key.len();
        let end = line[start..].find(' ').unwrap_or(line[start..].len());
        Some(&line[start..start + end])
    }
}
