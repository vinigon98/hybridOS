with open('rust-kernel/src/userland/shell.rs', 'r') as f:
    content = f.read()

# O bloco inteiro de cmd_download que precisa ser substituído
old_block = '''    pub fn cmd_download(&mut self, url: &str, dest_path: &str) -> Result<String, ShellError> {
        use crate::net::smoltcp_bridge::perform_curl;
        use crate::vfs::Permissions;

        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                      HYBRIDOS DOWNLOAD MANAGER                            \n");
        out.push_str("================================================================================\n");
        out.push_str(&format!("[URL] {}\n", url));
        out.push_str(&format!("[Destino] {}\n", dest_path));

        crate::serial_println!("[DOWNLOAD] Downloading from: {}", url);

        // Parse do endereço IP
        let ip: crate::compat::net::Ipv4Addr = url.parse()
            .map_err(|_| ShellError::DownloadError(format!("Invalid IP address: {}", url)))?;
        let port: u16 = 80;
        let path: &str = "/";

        crate::serial_println!("[DOWNLOAD] Connecting to {}:{}...", ip, port);

        // Baixa o conteúdo via HTTP GET
        let content = perform_curl(ip, port, path)
            .map_err(|e| ShellError::DownloadError(format!("Failed to download: {}", e)))?;

        crate::serial_println!("[DOWNLOAD] Downloaded {} bytes", content.len());

        // Salva no VFS
        vfs::vfs_create_file(dest_path, content.as_bytes(), Permissions::default_dir())
            .map_err(|e| ShellError::DownloadError(format!("Failed to save to VFS: {:?}", e)))?;

        crate::serial_println!("[DOWNLOAD] Saved successfully: {}", dest_path);

        out.push_str(&format!("[Tamanho] {} bytes\n", content.len()));
        out.push_str(&format!("[Status] Download concluído com sucesso\n"));
        out.push_str(&format!("\nPara executar: run {}\n", dest_path));

        Ok(out)
    }'''

new_block = '''    /// Command `download`: Baixa um arquivo via HTTP e salva no VFS.
    /// O parâmetro `url` deve ser um endereço IP (ex: "192.168.1.100").
    pub fn cmd_download(&mut self, url: &str, dest_path: &str) -> Result<String, ShellError> {
        use crate::net::smoltcp_bridge::perform_curl;
        use crate::vfs::Permissions;

        let mut out = String::new();
        out.push_str("================================================================================\n");
        out.push_str("                      HYBRIDOS DOWNLOAD MANAGER                            \n");
        out.push_str("================================================================================\n");
        out.push_str(&format!("[URL] {}\n", url));
        out.push_str(&format!("[Destino] {}\n", dest_path));

        crate::serial_println!("[DOWNLOAD] Downloading: {}", url);

        // Parse do endereço IP
        let ip_parts: Vec<&str> = url.split('.').collect();
        if ip_parts.len() != 4 {
            return Err(ShellError::DownloadError(format!("Invalid IP address: {}", url)));
        }
        let ip_bytes: [u8; 4] = [
            ip_parts[0].parse::<u8>().map_err(|_| ShellError::DownloadError(format!("Invalid IP address: {}", url)))?,
            ip_parts[1].parse::<u8>().map_err(|_| ShellError::DownloadError(format!("Invalid IP address: {}", url)))?,
            ip_parts[2].parse::<u8>().map_err(|_| ShellError::DownloadError(format!("Invalid IP address: {}", url)))?,
            ip_parts[3].parse::<u8>().map_err(|_| ShellError::DownloadError(format!("Invalid IP address: {}", url)))?,
        ];
        let ip = crate::compat::net::Ipv4Addr(ip_bytes);
        let port: u16 = 80;
        let path: &str = "/";

        crate::serial_println!("[DOWNLOAD] Connecting to {}:{}.{}.{}:{}...", ip.0[0], ip.0[1], ip.0[2], ip.0[3], port);

        // Baixa o conteúdo via HTTP GET
        // Converte o IP compat para o tipo usado pelo smoltcp_bridge
        let ip_net = crate::net::Ipv4Address(ip_bytes);
        let content = perform_curl(ip_net, port, path)
            .map_err(|e| ShellError::DownloadError(format!("Failed to download: {}", e)))?;

        crate::serial_println!("[DOWNLOAD] Downloaded {} bytes", content.len());

        // Salva no VFS
        vfs::vfs_create_file(dest_path, content.as_bytes(), Permissions::default_dir())
            .map_err(|e| ShellError::DownloadError(format!("Failed to save to VFS: {:?}", e)))?;

        crate::serial_println!("[DOWNLOAD] Saved successfully: {}", dest_path);

        out.push_str(&format!("[Tamanho] {} bytes\n", content.len()));
        out.push_str(&format!("[Status] Download concluído com sucesso\n"));
        out.push_str(&format!("\nPara executar: run {}\n", dest_path));

        Ok(out)
    }'''

if old_block in content:
    content = content.replace(old_block, new_block, 1)
    with open('rust-kernel/src/userland/shell.rs', 'w') as f:
        f.write(content)
    print('Sucesso: cmd_download corrigido!')
else:
    print('ERRO: Bloco não encontrado')
    # Mostrar o que está lá
    import re
    match = re.search(r'pub fn cmd_download.*?^    }', content, re.DOTALL)
    if match:
        print('Encontrado:', match.group(0)[:500])