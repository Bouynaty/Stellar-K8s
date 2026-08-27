pub struct SorobanMetrics<'a> {
    pub contract_id: &'a str,
    pub function_name: &'a str,
    pub cpu: u64,
    pub mem: u64,
}

pub struct LogParser;

impl LogParser {
    pub fn parse_line(line: &str) -> Option<SorobanMetrics<'_>> {
        let contract_id = Self::extract(line, "contract_id=")?;
        let function_name = Self::extract(line, "function=")?;
        let cpu = Self::extract(line, "cpu=")?.parse().ok()?;
        let mem = Self::extract(line, "mem=")?.parse().ok()?;

        Some(SorobanMetrics { contract_id, function_name, cpu, mem })
    }

    fn extract<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let start = line.find(key)? + key.len();
        let end = line[start..].find(' ').unwrap_or(line[start..].len());
        Some(&line[start..start + end])
    }
}
