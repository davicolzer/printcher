//! Fonte OpenDyslexic (`assets/fonts/`, licença SIL-OFL -- ver
//! `assets/fonts/OFL.txt`) embutida no binário via `include_bytes!`, no
//! mesmo espírito do mascote (`settings_window.rs::MASCOT_SVG`).
//!
//! Não dá pra simplesmente empacotar a fonte em `/app/share/fonts` dentro
//! do Flatpak -- testado ao vivo, essa pasta não está na lista que o
//! fontconfig escaneia dentro do sandbox. O que funciona (também testado
//! ao vivo: copiar um arquivo lá e o `fc-list` de dentro do sandbox já
//! reconhece na hora, sem precisar rodar `fc-cache`) é gravar em
//! `$XDG_DATA_HOME/fonts/` -- pasta que o próprio app já pode escrever
//! sem pedir permissão nova (é o diretório de dados privado dele), e que o
//! fontconfig já escaneia por padrão (`<dir prefix="xdg">fonts</dir>`),
//! tanto dentro do Flatpak quanto rodando o binário de dev direto (onde
//! cai no `~/.local/share/fonts` normal do usuário).

const REGULAR: &[u8] = include_bytes!("../assets/fonts/OpenDyslexic-Regular.otf");
const BOLD: &[u8] = include_bytes!("../assets/fonts/OpenDyslexic-Bold.otf");

/// Grava os arquivos da fonte em `$XDG_DATA_HOME/fonts/` se ainda não
/// estiverem lá -- chamar uma vez no início do daemon. Barato de chamar de
/// novo (só confere se o arquivo já existe), mas não precisa ser chamado
/// toda hora.
pub fn ensure_installed() -> anyhow::Result<()> {
    let dir = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("diretório de dados não encontrado"))?.join("fonts");
    std::fs::create_dir_all(&dir)?;

    for (name, bytes) in [("OpenDyslexic-Regular.otf", REGULAR), ("OpenDyslexic-Bold.otf", BOLD)] {
        let path = dir.join(name);
        if !path.exists() {
            std::fs::write(path, bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_installed_writes_both_font_files() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_data_home(tmp.path());

        ensure_installed().unwrap();

        let fonts_dir = tmp.path().join("fonts");
        assert!(fonts_dir.join("OpenDyslexic-Regular.otf").exists());
        assert!(fonts_dir.join("OpenDyslexic-Bold.otf").exists());
    }

    #[test]
    fn ensure_installed_does_not_overwrite_existing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let _guard = crate::testutil::set_xdg_data_home(tmp.path());

        ensure_installed().unwrap();
        let path = tmp.path().join("fonts").join("OpenDyslexic-Regular.otf");
        std::fs::write(&path, b"conteudo customizado").unwrap();

        ensure_installed().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"conteudo customizado", "não deveria reescrever um arquivo que já existe");
    }
}
