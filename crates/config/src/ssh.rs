use std::io::BufRead;

pub fn get_ssh_hosts() -> Vec<String> {
    let mut hosts = Vec::new();
    let ssh_config_path = dirs::home_dir().map(|h| h.join(".ssh/config"));

    if let Some(path) = ssh_config_path
        && path.exists()
            && let Ok(file) = std::fs::File::open(path) {
                let reader = std::io::BufReader::new(file);
                for line in reader.lines().map_while(Result::ok) {
                    let line = line.trim();
                    // Case insensitive check for "Host "
                    if line.to_lowercase().starts_with("host ") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        // parts[0] is "Host" (or "host")
                        if parts.len() > 1 {
                            for part in &parts[1..] {
                                // Exclude patterns containing wildcards
                                if !part.contains('*') && !part.contains('?') && !part.contains('!') {
                                    hosts.push(part.to_string());
                                }
                            }
                        }
                    }
                }
            }
    
    hosts
}
