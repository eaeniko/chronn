// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O idioma (rodada 23, 0.4.0): português (Brasil) e inglês.
//!
//! A interface monta quase todos os textos (`ui/js/textos.js`). Os que nascem aqui (as perguntas,
//! a bandeja, o título da janela, as mensagens de erro que chegam à tela e os nomes padrão do
//! primeiro uso) saem de uma tabela pequena por idioma (`PT` e `EN`, com as mesmas chaves), com os
//! parâmetros em `{0}`, `{1}`...
//!
//! A escolha fica na config (`ConfigApp::idioma`): `"sistema"` (o padrão), `"pt-BR"` ou `"en"`.
//! No "sistema", vale o idioma do Windows, que o app informa na abertura (`definir_sistema`):
//! começa com "pt" → português; qualquer outro → inglês.
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use crate::APP_NOME;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Idioma {
    Pt,
    En,
}

/// Valores de `ConfigApp::idioma`.
pub const IDIOMA_SISTEMA: &str = "sistema";
pub const IDIOMA_PT: &str = "pt-BR";
pub const IDIOMA_EN: &str = "en";

impl Idioma {
    /// O código que a interface usa ("pt-BR" ou "en").
    pub fn codigo(self) -> &'static str {
        match self {
            Idioma::Pt => IDIOMA_PT,
            Idioma::En => IDIOMA_EN,
        }
    }

    /// O idioma que vale: `escolha` da config e, no "sistema" (ou um valor desconhecido), o do
    /// Windows (`sistema`, como "pt-BR", "pt-PT", "en-US").
    pub fn resolver(escolha: &str, sistema: &str) -> Idioma {
        match escolha {
            IDIOMA_PT => Idioma::Pt,
            IDIOMA_EN => Idioma::En,
            _ => {
                let s = sistema.trim().to_ascii_lowercase();
                if s == "pt" || s.starts_with("pt-") || s.starts_with("pt_") {
                    Idioma::Pt
                } else {
                    Idioma::En
                }
            }
        }
    }
}

/// O idioma do Windows, informado pelo app na abertura. Sem ele (os testes), português.
static SISTEMA: OnceLock<String> = OnceLock::new();

/// Guarda o idioma do Windows ("pt-BR", "en-US"...). Só a primeira chamada vale.
pub fn definir_sistema(codigo: &str) {
    let _ = SISTEMA.set(codigo.to_string());
}

/// O idioma do Windows guardado ("pt-BR" sem nenhum).
pub fn sistema() -> &'static str {
    SISTEMA.get().map_or(IDIOMA_PT, String::as_str)
}

/// O idioma que vale para a escolha `escolha` da config, com o Windows de agora.
pub fn da_escolha(escolha: &str) -> Idioma {
    Idioma::resolver(escolha, sistema())
}

/// O idioma de agora no app (a bandeja, os títulos e os erros leem daqui): 0 = português, 1 =
/// inglês. Quem monta o estado (`Nucleo::snapshot`) atualiza.
static ATUAL: AtomicU8 = AtomicU8::new(0);

pub fn atual() -> Idioma {
    if ATUAL.load(Ordering::Relaxed) == 1 { Idioma::En } else { Idioma::Pt }
}

/// Passa a usar `i`. Devolve se mudou.
pub fn usar(i: Idioma) -> bool {
    let novo = if i == Idioma::En { 1 } else { 0 };
    ATUAL.swap(novo, Ordering::Relaxed) != novo
}

/// O texto `chave` no idioma `i` (sem parâmetros). Chave que não existe: a própria chave.
pub fn t(i: Idioma, chave: &str) -> &'static str {
    let tabela = match i {
        Idioma::Pt => PT,
        Idioma::En => EN,
    };
    tabela.iter().find(|(k, _)| *k == chave).map(|(_, v)| *v).unwrap_or_else(|| {
        // nunca deveria acontecer: o teste confere as chaves dos dois idiomas
        PT.iter().find(|(k, _)| *k == chave).map_or("", |(_, v)| *v)
    })
}

/// O texto `chave` no idioma `i` com os parâmetros no lugar de `{0}`, `{1}`... e o nome do app
/// no de `{app}`.
pub fn tf(i: Idioma, chave: &str, args: &[&str]) -> String {
    let mut s = t(i, chave).replace("{app}", APP_NOME);
    for (n, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{n}}}"), a);
    }
    s
}

/// `t` no idioma de agora.
pub fn ta(chave: &str) -> &'static str {
    t(atual(), chave)
}

/// `tf` no idioma de agora.
pub fn tfa(chave: &str, args: &[&str]) -> String {
    tf(atual(), chave, args)
}

/// "25 min" ou "8h 05min" ("8h 05m" em inglês): a duração das perguntas.
pub fn duracao_min(i: Idioma, min: i64) -> String {
    if min < 60 {
        tf(i, "dur.min", &[&min.to_string()])
    } else {
        tf(i, "dur.h_min", &[&(min / 60).to_string(), &format!("{:02}", min % 60)])
    }
}

/// "A, B e C" ("A, B and C").
pub fn lista(i: Idioma, nomes: &[String]) -> String {
    match nomes {
        [] => String::new(),
        [um] => um.clone(),
        [inicio @ .., ultimo] => format!("{}{}{ultimo}", inicio.join(", "), t(i, "lista.e")),
    }
}

/// Os nomes padrão do primeiro uso (o banco novo): as atividades Trabalho, Reunião e Estudo e o
/// primeiro espaço. Depois de criados, são dados da pessoa: não mudam com o idioma.
pub fn nome_padrao(i: Idioma, qual: &str) -> &'static str {
    t(i, qual)
}

const PT: &[(&str, &str)] = &[
    // durações e listas
    ("dur.min", "{0} min"),
    ("dur.h_min", "{0}h {1}min"),
    ("lista.e", " e "),
    // nomes padrão do primeiro uso
    ("padrao.trabalho", "Trabalho"),
    ("padrao.reuniao", "Reunião"),
    ("padrao.estudo", "Estudo"),
    ("padrao.geral", "Geral"),
    ("padrao.sem_grupo", "Sem grupo"),
    // as perguntas
    ("pg.pausado", "Deixar pausado"),
    ("pg.contar_desde", "Contar desde {0}"),
    ("pg.retomar_agora", "Retomar agora"),
    ("pg.contar_fora", "Contar o tempo fora"),
    ("pg.outro_timer", "Outro timer…"),
    ("pg.novo_timer", "+ Novo timer…"),
    ("pg.nao_contar", "Não contar"),
    ("pg.conta_nova", "De qual espaço é esta conta?"),
    ("pg.depois", "Depois"),
    ("erro.conta_desconhecida", "Esta conta do Claude não está mais na lista."),
    ("erro.wsl_desconhecida", "Distro do WSL desconhecida: {0}"),
    ("erro.so_perfil_normal", "No perfil de teste, o WSL de verdade não muda."),
    ("erro.wsl_parada", "A distro {0} está parada: abra o WSL para ligar."),
    ("erro.wsl_nao_subiu", "Não consegui ligar a distro {0}. Abra o WSL e tente de novo."),
    ("erro.wsl_parada_desligar", "A distro {0} está parada: abra o WSL para desligar."),
    ("erro.wsl_sem_claude", "O Claude Code não foi encontrado na distro {0} (falta a pasta ~/.claude)."),
    ("erro.wsl_sem_home", "Não deu para achar a pasta do usuário na distro {0}."),
    ("erro.wsl_sem_hook", "O hook para Linux não veio com o app: {0}"),
    ("erro.wsl_chmod", "Não deu para tornar o hook executável na distro {0}: {1}"),
    ("erro.wsl_nao_respondeu", "O WSL não respondeu a tempo."),
    ("erro.wsl_hook_invalido", "O hook para Linux do app não é um executável do Linux: {0}"),
    ("sessao.chat_sem_nome", "Conversa do Chat"),
    ("pg.sessao_nova", "Sessão nova do Claude na pasta {0}. Em qual timer contar?"),
    ("pg.sessao_nova_sem_pasta", "Sessão nova do Claude. Em qual timer contar?"),
    ("pg.contar_em", "Contar em {0}"),
    ("pg.escolher_o_timer", "Escolher o timer…"),
    ("erro.pasta_fora_da_lista", "Essa pasta não é uma das pastas desta sessão."),
    ("pg.timer_da_regra", "o timer da regra"),
    ("pg.conversa_nova", "Conversa nova: {0} · contando em {1} pela pasta"),
    ("pg.certo", "Certo"),
    ("pg.ligado", "ligado"),
    ("pg.ligados", "ligados"),
    ("pg.inat_antes", "Antes da virada do dia, você ficou {0} sem mexer no PC com {1} {2}. O tempo daquele dia fica como está."),
    ("pg.inat_contar", "Você ficou {0} sem mexer no PC. Contar em {1}{2}?"),
    ("pg.inat_retomar", "Você ficou {0} sem mexer no PC. Retomar {1}?"),
    ("pg.fechado_antes", "Antes da virada do dia, o {app} ficou fechado {0} com {1} {2}. O tempo daquele dia fica como está."),
    ("pg.fechado", "O {app} ficou fechado {0} com {1} {2}. {3}?"),
    ("pg.contar", "Contar{0}"),
    ("pg.retomar", "Retomar"),
    ("pg.desde_o_corte", " desde {0} ({1})"),
    // os avisos de ligada sozinha
    ("aviso.pela_pasta", "Ligada a {0} pela pasta"),
    ("aviso.pela_conversa", "Ligada a {0} pela conversa anterior"),
    // a bandeja e os títulos
    ("bandeja.abrir", "Abrir"),
    ("bandeja.pausar_tudo", "Pausar tudo"),
    ("bandeja.dock_agora", "Mostrar o dock agora"),
    ("bandeja.dock_alternar", "Mostrar/esconder dock"),
    ("bandeja.sobre", "Sobre o {app}"),
    ("bandeja.sair", "Sair"),
    ("bandeja.rodando", "{0} · {1} rodando"),
    ("janela.rodando", "● {0} rodando · {1}"),
    ("janela.escolher_pasta", "Escolher a pasta"),
    // erros que chegam à tela
    ("erro.nome_vazio", "O nome não pode ficar vazio"),
    ("erro.corte", "Use HH:MM, de 00:00 a 23:59"),
    ("erro.link", "link desconhecido"),
    ("erro.abrir_link", "não consegui abrir o link: {0}"),
    ("erro.abrir_pasta", "não consegui abrir a pasta: {0}"),
    ("erro.so_perfil_teste", "Só existe no perfil de teste"),
    ("erro.nao_e_perfil_teste", "A pasta não é a do perfil de teste: nada foi apagado"),
    ("erro.apagar_perfil", "não consegui apagar o perfil de teste: {0}"),
    ("erro.seletor_parou", "O seletor de pasta parou"),
    ("erro.seletor_so_windows", "Seletor de pasta só no Windows"),
    ("erro.seletor_abrir", "Não deu para abrir o seletor de pasta: {0}"),
    ("erro.mova_os_grupos", "Mova os grupos para outro espaço antes"),
    ("erro.ultimo_espaco", "Precisa sobrar pelo menos um espaço"),
    ("erro.desfazer_ja_mudou", "Não dá mais para desfazer: ele já mudou de lugar"),
    ("erro.padrao_nao_oculta", "A atividade padrão do timer não pode ser ocultada"),
    ("erro.banco_nao_abriu", "O banco não abriu ({0}). Feche e abra o {app}; os backups estão na pasta de dados."),
    ("erro.guardar_banco", "não consegui guardar o banco de agora: {0}"),
    ("erro.por_banco", "não consegui pôr o banco novo no lugar: {0}"),
    ("erro.banco_novo", "o banco novo não abriu: {0}"),
    ("erro.de_antes_tambem", "{0}; o de antes também não abriu: {1}"),
    ("erro.nada_mudou", "{0}. Nada mudou."),
    ("erro.backup_desconhecido", "backup desconhecido: {0}"),
    ("erro.backup_sumiu", "Esse backup não existe mais"),
    ("erro.guardar_antes_restaurar", "não consegui guardar o estado de agora antes de restaurar: {0}"),
    ("erro.timer_nao_encontrado", "Timer não encontrado"),
    ("erro.atividade_nao_encontrada", "Atividade não encontrada"),
    ("erro.grupo_nao_encontrado", "Grupo não encontrado"),
    ("erro.espaco_nao_encontrado", "Espaço não encontrado"),
    ("erro.ajuste_nao_encontrado", "Ajuste não encontrado"),
    ("erro.nao_da_desfazer", "Não dá para desfazer: {0}"),
    ("erro.grupo_arquivado", "Não dá para desfazer: o grupo de antes foi arquivado"),
    ("erro.espaco_arquivado", "O espaço {0} está arquivado"),
    ("erro.nome_max", "O nome vai até {0} letras"),
    ("erro.espaco_existe", "Já existe o espaço {0}"),
    ("erro.ajuste_minutos", "Diga quantos minutos a mais ou a menos"),
    ("erro.ajuste_24h", "O ajuste vai até 24 h, para mais ou para menos"),
    ("erro.motivo_max", "O motivo vai até {0} letras"),
    ("erro.tempos_minimo", "Os tempos precisam ser de pelo menos 1 minuto ({0})"),
    ("erro.inatividade_faixa", "A inatividade vai de 1 a 30 minutos"),
    ("erro.tempos_24h", "Uso recente e sessão parada vão até 24 h"),
    ("tempo.uso", "uso recente"),
    ("tempo.inatividade", "inatividade"),
    ("tempo.sessao_parada", "sessão parada"),
    ("erro.pasta_larga", "Uma regra não pode ficar na raiz de um drive nem na pasta do usuário: pegaria tudo o que está embaixo."),
    ("erro.pasta_ignorada", "Esta pasta está em \"Pastas que nunca contam\": as conversas nela não contam."),
    ("erro.ler", "não consegui ler: {0}"),
    ("erro.json", "JSON inválido, nada foi alterado: {0}"),
    ("erro.json_raiz", "JSON inválido, nada foi alterado: a raiz não é um objeto"),
    ("erro.criar_pasta", "não consegui criar a pasta: {0}"),
    ("erro.gravar", "não consegui gravar: {0}"),
    ("erro.copia_seguranca", "não consegui fazer a cópia de segurança: {0}"),
    ("erro.backup_fazer", "não consegui fazer o backup: {0}"),
    ("erro.backup_pasta", "não consegui criar a pasta dos backups: {0}"),
    ("erro.backup_abrir", "não consegui abrir o backup: {0}"),
    ("erro.backup_nao_e_banco", "o backup não é um banco do {app}"),
    ("erro.backup_copiar", "não consegui copiar o backup: {0}"),
    ("erro.copia_nao_confere", "a cópia do banco não confere: {0}"),
    ("erro.copia_abrir", "não consegui abrir a cópia do banco: {0}"),
    ("hooks.sem", "(sem hooks)"),
    ("hooks.antes", "Antes"),
    ("hooks.depois", "Depois"),
    ("erro.atualizar_procurar", "a busca de atualizações falhou: {0}"),
    ("erro.atualizar_procurar_rede", "a busca de atualizações falhou (sem internet?)"),
    ("erro.atualizar_baixar", "o download da atualização falhou: {0}"),
    ("erro.atualizar_baixar_rede", "o download da atualização falhou (sem internet?)"),
    ("erro.atualizar_assinatura", "a assinatura da atualização não confere, e nada foi instalado"),
    ("erro.atualizar_nenhuma", "nenhuma atualização disponível"),
    ("erro.atualizar_teste", "no perfil de teste, a instalação de atualizações fica desligada"),
    ("erro.escolha_outro_grupo", "Escolha outro grupo: {0} está arquivado."),
    ("erro.atividade_arquivada", "A atividade {0} está arquivada"),
    ("erro.atividade_existe", "Já existe a atividade {0}"),
    ("erro.ultima_atividade", "Precisa sobrar pelo menos uma atividade"),
    ("erro.atividade_padrao_de", "{0} é a atividade padrão de {1}"),
    ("erro.regra_nao_encontrada", "Regra não encontrada"),
    ("erro.pasta_ja_e_de", "A pasta já é de {0}."),
    ("erro.sessao_nao_encontrada", "Sessão não encontrada"),
];

const EN: &[(&str, &str)] = &[
    ("dur.min", "{0} min"),
    ("dur.h_min", "{0}h {1}m"),
    ("lista.e", " and "),
    ("padrao.trabalho", "Work"),
    ("padrao.reuniao", "Meeting"),
    ("padrao.estudo", "Study"),
    ("padrao.geral", "General"),
    ("padrao.sem_grupo", "No group"),
    ("pg.pausado", "Leave paused"),
    ("pg.contar_desde", "Count from {0}"),
    ("pg.retomar_agora", "Resume now"),
    ("pg.contar_fora", "Count the time away"),
    ("pg.outro_timer", "Another timer…"),
    ("pg.novo_timer", "+ New timer…"),
    ("pg.nao_contar", "Don’t count"),
    ("pg.conta_nova", "Which space is this account in?"),
    ("pg.depois", "Later"),
    ("erro.conta_desconhecida", "This Claude account is no longer on the list."),
    ("erro.wsl_desconhecida", "Unknown WSL distro: {0}"),
    ("erro.so_perfil_normal", "In the test profile, the real WSL doesn’t change."),
    ("erro.wsl_parada", "The {0} distro is stopped: open WSL to connect it."),
    ("erro.wsl_nao_subiu", "Couldn't start the {0} distro. Open WSL and try again."),
    ("erro.wsl_parada_desligar", "The {0} distro is stopped: open WSL to disconnect it."),
    ("erro.wsl_sem_claude", "Claude Code was not found in the {0} distro (no ~/.claude folder)."),
    ("erro.wsl_sem_home", "Couldn’t find the user folder in the {0} distro."),
    ("erro.wsl_sem_hook", "The Linux hook didn’t come with the app: {0}"),
    ("erro.wsl_chmod", "Couldn’t make the hook executable in the {0} distro: {1}"),
    ("erro.wsl_nao_respondeu", "WSL didn’t answer in time."),
    ("erro.wsl_hook_invalido", "The app’s Linux hook isn’t a Linux executable: {0}"),
    ("sessao.chat_sem_nome", "Chat conversation"),
    ("pg.sessao_nova", "New Claude session in the folder {0}. Which timer should count it?"),
    ("pg.sessao_nova_sem_pasta", "New Claude session. Which timer should count it?"),
    ("pg.contar_em", "Count in {0}"),
    ("pg.escolher_o_timer", "Choose the timer…"),
    ("erro.pasta_fora_da_lista", "That folder isn’t one of this session’s folders."),
    ("pg.timer_da_regra", "the rule’s timer"),
    ("pg.conversa_nova", "New conversation: {0} · counting in {1} by folder"),
    ("pg.certo", "OK"),
    ("pg.ligado", "running"),
    ("pg.ligados", "running"),
    ("pg.inat_antes", "Before the day rolled over, you were away from the PC for {0} with {1} {2}. That day’s time stays as it is."),
    ("pg.inat_contar", "You were away from the PC for {0}. Count it in {1}{2}?"),
    ("pg.inat_retomar", "You were away from the PC for {0}. Resume {1}?"),
    ("pg.fechado_antes", "Before the day rolled over, {app} was closed for {0} with {1} {2}. That day’s time stays as it is."),
    ("pg.fechado", "{app} was closed for {0} with {1} {2}. {3}?"),
    ("pg.contar", "Count it{0}"),
    ("pg.retomar", "Resume"),
    ("pg.desde_o_corte", " from {0} ({1})"),
    ("aviso.pela_pasta", "Linked to {0} by folder"),
    ("aviso.pela_conversa", "Linked to {0} by the previous conversation"),
    ("bandeja.abrir", "Open"),
    ("bandeja.pausar_tudo", "Pause all"),
    ("bandeja.dock_agora", "Show the dock now"),
    ("bandeja.dock_alternar", "Show/hide dock"),
    ("bandeja.sobre", "About {app}"),
    ("bandeja.sair", "Quit"),
    ("bandeja.rodando", "{0} · {1} running"),
    ("janela.rodando", "● {0} running · {1}"),
    ("janela.escolher_pasta", "Choose the folder"),
    ("erro.nome_vazio", "The name can’t be empty"),
    ("erro.corte", "Use HH:MM, from 00:00 to 23:59"),
    ("erro.link", "unknown link"),
    ("erro.abrir_link", "couldn’t open the link: {0}"),
    ("erro.abrir_pasta", "couldn’t open the folder: {0}"),
    ("erro.so_perfil_teste", "Only available in the test profile"),
    ("erro.nao_e_perfil_teste", "The folder isn’t the test profile’s: nothing was deleted"),
    ("erro.apagar_perfil", "couldn’t delete the test profile: {0}"),
    ("erro.seletor_parou", "The folder picker stopped"),
    ("erro.seletor_so_windows", "Folder picker only on Windows"),
    ("erro.seletor_abrir", "Couldn’t open the folder picker: {0}"),
    ("erro.mova_os_grupos", "Move the groups to another space first"),
    ("erro.ultimo_espaco", "At least one space must remain"),
    ("erro.desfazer_ja_mudou", "Can’t undo anymore: it has already moved"),
    ("erro.padrao_nao_oculta", "The timer’s default activity can’t be hidden"),
    ("erro.banco_nao_abriu", "The database didn’t open ({0}). Close and reopen {app}; the backups are in the data folder."),
    ("erro.guardar_banco", "couldn’t save a copy of the current database: {0}"),
    ("erro.por_banco", "couldn’t put the new database in place: {0}"),
    ("erro.banco_novo", "the new database didn’t open: {0}"),
    ("erro.de_antes_tambem", "{0}; the previous one didn’t open either: {1}"),
    ("erro.nada_mudou", "{0}. Nothing changed."),
    ("erro.backup_desconhecido", "unknown backup: {0}"),
    ("erro.backup_sumiu", "That backup no longer exists"),
    ("erro.guardar_antes_restaurar", "couldn’t save the current state before restoring: {0}"),
    ("erro.timer_nao_encontrado", "Timer not found"),
    ("erro.atividade_nao_encontrada", "Activity not found"),
    ("erro.grupo_nao_encontrado", "Group not found"),
    ("erro.espaco_nao_encontrado", "Space not found"),
    ("erro.ajuste_nao_encontrado", "Adjustment not found"),
    ("erro.nao_da_desfazer", "Can’t undo: {0}"),
    ("erro.grupo_arquivado", "Can’t undo: the previous group was archived"),
    ("erro.espaco_arquivado", "The space {0} is archived"),
    ("erro.nome_max", "The name can have up to {0} characters"),
    ("erro.espaco_existe", "The space {0} already exists"),
    ("erro.ajuste_minutos", "Enter how many minutes to add or subtract"),
    ("erro.ajuste_24h", "The adjustment can’t exceed 24 h either way"),
    ("erro.motivo_max", "The reason can have up to {0} characters"),
    ("erro.tempos_minimo", "The times must be at least 1 minute ({0})"),
    ("erro.inatividade_faixa", "Inactivity goes from 1 to 30 minutes"),
    ("erro.tempos_24h", "Recent use and stopped session go up to 24 h"),
    ("tempo.uso", "recent use"),
    ("tempo.inatividade", "inactivity"),
    ("tempo.sessao_parada", "stopped session"),
    ("erro.pasta_larga", "A rule can’t sit at the root of a drive or in the user folder: it would catch everything below."),
    ("erro.pasta_ignorada", "This folder is in “Folders that never count”: conversations in it don’t count."),
    ("erro.ler", "couldn’t read: {0}"),
    ("erro.json", "Invalid JSON, nothing was changed: {0}"),
    ("erro.json_raiz", "Invalid JSON, nothing was changed: the root isn’t an object"),
    ("erro.criar_pasta", "couldn’t create the folder: {0}"),
    ("erro.gravar", "couldn’t write: {0}"),
    ("erro.copia_seguranca", "couldn’t make the safety copy: {0}"),
    ("erro.backup_fazer", "couldn’t make the backup: {0}"),
    ("erro.backup_pasta", "couldn’t create the backups folder: {0}"),
    ("erro.backup_abrir", "couldn’t open the backup: {0}"),
    ("erro.backup_nao_e_banco", "the backup isn’t a {app} database"),
    ("erro.backup_copiar", "couldn’t copy the backup: {0}"),
    ("erro.copia_nao_confere", "the database copy failed the integrity check: {0}"),
    ("erro.copia_abrir", "couldn’t open the database copy: {0}"),
    ("hooks.sem", "(no hooks)"),
    ("hooks.antes", "Before"),
    ("hooks.depois", "After"),
    ("erro.atualizar_procurar", "the update check failed: {0}"),
    ("erro.atualizar_procurar_rede", "the update check failed (no internet?)"),
    ("erro.atualizar_baixar", "the update download failed: {0}"),
    ("erro.atualizar_baixar_rede", "the update download failed (no internet?)"),
    ("erro.atualizar_assinatura", "the update’s signature doesn’t match, and nothing was installed"),
    ("erro.atualizar_nenhuma", "no update available"),
    ("erro.atualizar_teste", "in the test profile, installing updates is turned off"),
    ("erro.escolha_outro_grupo", "Choose another group: {0} is archived."),
    ("erro.atividade_arquivada", "The activity {0} is archived"),
    ("erro.atividade_existe", "The activity {0} already exists"),
    ("erro.ultima_atividade", "At least one activity must remain"),
    ("erro.atividade_padrao_de", "{0} is the default activity of {1}"),
    ("erro.regra_nao_encontrada", "Rule not found"),
    ("erro.pasta_ja_e_de", "The folder already belongs to {0}."),
    ("erro.sessao_nao_encontrada", "Session not found"),
];

#[cfg(test)]
mod testes {
    use super::*;

    fn marcas(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s.match_indices('{').map(|(i, _)| s[i..].split('}').next().unwrap_or("").to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn os_dois_idiomas_tem_as_mesmas_chaves_e_os_mesmos_parametros() {
        let chaves = |t: &'static [(&'static str, &'static str)]| t.iter().map(|(k, _)| *k).collect::<Vec<&str>>();
        assert_eq!(chaves(PT), chaves(EN), "mesmas chaves, na mesma ordem");
        let mut vistas = std::collections::BTreeSet::new();
        for ((k, pt), (_, en)) in PT.iter().zip(EN) {
            assert!(vistas.insert(*k), "chave repetida: {k}");
            assert_eq!(marcas(pt), marcas(en), "parâmetros de {k}");
            assert!(!pt.is_empty() && !en.is_empty(), "{k} vazia");
        }
    }

    #[test]
    fn o_sistema_escolhe_pelo_windows() {
        assert_eq!(Idioma::resolver("sistema", "pt-BR"), Idioma::Pt);
        assert_eq!(Idioma::resolver("sistema", "pt-PT"), Idioma::Pt);
        assert_eq!(Idioma::resolver("sistema", "pt"), Idioma::Pt);
        assert_eq!(Idioma::resolver("sistema", "en-US"), Idioma::En);
        assert_eq!(Idioma::resolver("sistema", "es-ES"), Idioma::En, "outro idioma cai no inglês");
        assert_eq!(Idioma::resolver("sistema", ""), Idioma::En);
        // a escolha manda, qualquer que seja o Windows
        assert_eq!(Idioma::resolver("en", "pt-BR"), Idioma::En);
        assert_eq!(Idioma::resolver("pt-BR", "en-US"), Idioma::Pt);
        // valor desconhecido: como o "sistema"
        assert_eq!(Idioma::resolver("fr", "pt-BR"), Idioma::Pt);
    }

    #[test]
    fn parametros_duracoes_e_listas() {
        assert_eq!(tf(Idioma::Pt, "bandeja.sobre", &[]), format!("Sobre o {APP_NOME}"));
        assert_eq!(tf(Idioma::En, "bandeja.sobre", &[]), format!("About {APP_NOME}"));
        assert_eq!(tf(Idioma::En, "pg.inat_retomar", &["26 min", "A"]), "You were away from the PC for 26 min. Resume A?");
        assert_eq!(duracao_min(Idioma::Pt, 26), "26 min");
        assert_eq!(duracao_min(Idioma::Pt, 121), "2h 01min");
        assert_eq!(duracao_min(Idioma::En, 121), "2h 01m");
        let n = ["A".to_string(), "B".to_string(), "C".to_string()];
        assert_eq!(lista(Idioma::Pt, &n), "A, B e C");
        assert_eq!(lista(Idioma::En, &n), "A, B and C");
        assert_eq!(nome_padrao(Idioma::En, "padrao.reuniao"), "Meeting");
        assert_eq!(tf(Idioma::En, "erro.backup_nao_e_banco", &[]), format!("the backup isn’t a {APP_NOME} database"));
    }
}
