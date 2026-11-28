// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Spec 004: arquivar com destino (as conversas e as regras de pasta), os arquivados (ver e
//! desarquivar) e o apagar de verdade (sem horas, com o "Desfazer"; com horas, com o nome e um
//! backup antes).
use std::collections::BTreeSet;
use std::path::Path;

use chrono::NaiveDateTime;
use nucleo::banco::{Apagado, TempoGuardado};
use nucleo::idioma::{ta, tfa};
use nucleo::modelo::{Ligacao, Mudanca};
use nucleo::tempo::Ms;
use serde::Serialize;

use crate::estado_app::{texto, Nucleo};

/// O que o diálogo de arquivar ou de apagar mostra de um timer ou de um grupo.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UsoDto {
    /// Quantos timers (o grupo; o timer, 1).
    pub timers: usize,
    /// As conversas recentes ligadas a eles.
    pub conversas: usize,
    /// As pastas das regras deles.
    pub regras: Vec<String>,
    /// O tempo guardado de sempre e em quantos dias, do primeiro ao último (AAAA-MM-DD).
    pub total_ms: Ms,
    pub dias: usize,
    pub primeiro_dia: Option<String>,
    pub ultimo_dia: Option<String>,
    /// Algo rodando neles agora.
    pub rodando: bool,
    /// O destino sugerido para as conversas e as regras: o timer à vista do mesmo grupo com a
    /// conversa mais recente (nulo: "Não contar mais").
    pub sugestao: Option<i64>,
}

/// Um timer na lista dos arquivados.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TimerArquivadoDto {
    pub id: i64,
    pub nome: String,
    pub cor: String,
    pub arquivado: bool,
    pub total_ms: Ms,
    /// O grupo (à vista) e o espaço dele.
    pub grupo_nome: Option<String>,
    pub espaco_id: i64,
}

/// Um grupo arquivado, com os timers dele.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GrupoArquivadoDto {
    pub id: i64,
    pub nome: String,
    pub cor: String,
    pub espaco_id: i64,
    pub total_ms: Ms,
    pub timers: Vec<TimerArquivadoDto>,
}

/// Ajustes → Arquivados: os grupos arquivados e os timers arquivados (fora deles). Os de um espaço
/// arquivado ficam de fora: o espaço desarquiva em "Grupos e regras".
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ArquivadosDto {
    pub grupos: Vec<GrupoArquivadoDto>,
    pub timers: Vec<TimerArquivadoDto>,
}

/// O nome digitado confere com o do timer ou do grupo (sem diferenciar maiúsculas e os espaços
/// das pontas)?
pub fn nome_confere(digitado: &str, nome: &str) -> bool {
    let n = |s: &str| s.trim().to_lowercase();
    !n(digitado).is_empty() && n(digitado) == n(nome)
}

impl Nucleo {
    /// Passa ao motor os escondidos de agora (spec 004): o que roda num deles fecha e as sessões
    /// ligadas a eles ficam sem timer. Também relê as regras e as atividades padrão.
    pub fn reesconder(&mut self, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.motor.regras_pasta = self.banco.regras_pasta().map_err(texto)?;
        self.motor.atividades_padrao = self.banco.atividades_padrao().map_err(texto)?;
        let escondidos = self.banco.timers_escondidos().map_err(texto)?;
        Ok(self.motor.esconder(escondidos, agora))
    }

    /// Os timers de um grupo (os arquivados também).
    fn timers_do_grupo(&self, grupo: i64) -> Result<Vec<i64>, String> {
        Ok(self.banco.timers_com(true).map_err(texto)?.into_iter().filter(|t| t.grupo_id == Some(grupo)).map(|t| t.id).collect())
    }

    fn uso(&self, ids: &[i64], grupo: Option<i64>, agora: Ms) -> Result<UsoDto, String> {
        let (corte, fuso) = (self.motor.cfg.corte_min, self.motor.cfg.fuso_min);
        let tempo = self.banco.tempo_guardado(agora, corte, fuso).map_err(texto)?;
        let mut g = TempoGuardado::default();
        for t in ids.iter().filter_map(|id| tempo.get(id)) {
            g.total_ms += t.total_ms;
            g.dias.extend(t.dias.iter().copied());
        }
        let dele = |t: Option<i64>| t.is_some_and(|t| ids.contains(&t));
        let regras: Vec<String> =
            self.banco.regras_pasta().map_err(texto)?.into_iter().filter(|r| ids.contains(&r.timer_id)).map(|r| r.pasta).collect();
        // a sugestão: um timer à vista do mesmo grupo, o da conversa mais recente
        let vizinhos: BTreeSet<i64> = match grupo {
            Some(gr) => self
                .banco
                .timers()
                .map_err(texto)?
                .into_iter()
                .filter(|t| t.grupo_id == Some(gr) && !ids.contains(&t.id) && !self.motor.escondido(Some(t.id)))
                .map(|t| t.id)
                .collect(),
            None => BTreeSet::new(),
        };
        let sugestao = self
            .motor
            .sessoes
            .values()
            .filter(|s| s.timer_id.is_some_and(|t| vizinhos.contains(&t)))
            .max_by_key(|s| s.ultimo_evento)
            .and_then(|s| s.timer_id);
        Ok(UsoDto {
            timers: ids.len(),
            conversas: self.motor.sessoes.values().filter(|s| dele(s.timer_id)).count(),
            regras,
            total_ms: g.total_ms,
            dias: g.dias.len(),
            primeiro_dia: g.dias.first().map(|d| d.format("%Y-%m-%d").to_string()),
            ultimo_dia: g.dias.last().map(|d| d.format("%Y-%m-%d").to_string()),
            rodando: self.motor.abertos.iter().any(|a| dele(a.timer_id)),
            sugestao,
        })
    }

    /// O que o diálogo de arquivar ou apagar um timer mostra.
    pub fn uso_do_timer(&self, id: i64, agora: Ms) -> Result<UsoDto, String> {
        let t = self.timer_qualquer(id)?;
        self.uso(&[id], t.grupo_id, agora)
    }

    /// O que o diálogo de arquivar ou apagar um grupo mostra (os timers dele, os arquivados também).
    pub fn uso_do_grupo(&self, id: i64, agora: Ms) -> Result<UsoDto, String> {
        self.grupo_qualquer(id)?;
        self.uso(&self.timers_do_grupo(id)?, None, agora)
    }

    fn timer_qualquer(&self, id: i64) -> Result<nucleo::modelo::Timer, String> {
        self.banco
            .timers_com(true)
            .map_err(texto)?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| ta("erro.timer_nao_encontrado").to_string())
    }

    fn grupo_qualquer(&self, id: i64) -> Result<nucleo::modelo::Grupo, String> {
        self.banco
            .grupos_com(true)
            .map_err(texto)?
            .into_iter()
            .find(|g| g.id == id)
            .ok_or_else(|| ta("erro.grupo_nao_encontrado").to_string())
    }

    /// O destino das conversas e das regras de `ids` (spec 004): com `destino`, as sessões em
    /// memória passam a ele (a que trabalha segue contando lá, sem buraco) e as do banco e as regras
    /// também; sem, as do banco ficam sem timer e as regras saem (as em memória, o `esconder`).
    fn dar_destino(&mut self, ids: &[i64], destino: Option<i64>, agora: Ms) -> Result<(), String> {
        if let Some(d) = destino {
            if ids.contains(&d) {
                return Err(ta("erro.destino_invalido").into());
            }
            self.checar_timer(d)?;
            let ligadas: Vec<String> =
                self.motor.sessoes.values().filter(|s| s.timer_id.is_some_and(|t| ids.contains(&t))).map(|s| s.id.clone()).collect();
            let mut mud = vec![];
            for s in ligadas {
                mud.extend(self.motor.ligar_sessao(&s, d, false, agora));
            }
            self.aplicar(&mud)?;
        }
        self.banco.mover_ligacoes(ids, destino, agora).map_err(texto)
    }

    /// Arquiva o timer, com o destino das conversas e das regras dele (`destino`: o timer para
    /// onde vão; nulo, "Não contar mais").
    pub fn arquivar_timer(&mut self, id: i64, destino: Option<i64>, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_banco()?;
        let mut t = self.timer_qualquer(id)?;
        if t.arquivado {
            return Err(ta("erro.ja_arquivado").into());
        }
        let feito = self.dar_destino(&[id], destino, agora).and_then(|_| {
            t.arquivado = true;
            self.banco.editar_timer(&t).map_err(texto)
        });
        self.depois_de_arquivar(feito, agora)
    }

    /// O fim do arquivar (e do apagar): o motor e o banco voltam a concordar, mesmo se uma etapa
    /// falhou no meio (revisão M3). Tudo já foi aplicado por dentro: devolve vazio (o `agir` não
    /// aplica de novo; revisão I1).
    fn depois_de_arquivar(&mut self, feito: Result<(), String>, agora: Ms) -> Result<Vec<Mudanca>, String> {
        let m = self.reesconder(agora);
        let aplicado = m.and_then(|m| self.aplicar(&m));
        feito?;
        aplicado?;
        Ok(vec![])
    }

    /// Arquiva o grupo (e com ele os timers), com o destino das conversas e das regras deles.
    pub fn arquivar_grupo(&mut self, id: i64, destino: Option<i64>, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_banco()?;
        let mut g = self.grupo_qualquer(id)?;
        if g.arquivado {
            return Err(ta("erro.ja_arquivado").into());
        }
        let ids = self.timers_do_grupo(id)?;
        let feito = self.dar_destino(&ids, destino, agora).and_then(|_| {
            g.arquivado = true;
            self.banco.editar_grupo(&g).map_err(texto)
        });
        self.depois_de_arquivar(feito, agora)
    }

    /// O nome do espaço, se ele está arquivado.
    fn espaco_arquivado(&self, espaco: i64) -> Result<Option<String>, String> {
        Ok(self.banco.espacos().map_err(texto)?.into_iter().find(|e| e.id == espaco && e.arquivado).map(|e| e.nome))
    }

    /// Desarquiva o timer. O de um grupo ou espaço arquivado recusa (desarquive o grupo ou o espaço
    /// antes). As conversas e as regras que ele tinha não voltam.
    pub fn desarquivar_timer(&mut self, id: i64, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_banco()?;
        let mut t = self.timer_qualquer(id)?;
        if !t.arquivado {
            return Err(ta("erro.nao_arquivado").into());
        }
        let espaco = match t.grupo_id {
            Some(gid) => {
                let g = self.grupo_qualquer(gid)?;
                if g.arquivado {
                    return Err(tfa("erro.desarquive_o_grupo", &[&g.nome]));
                }
                g.espaco_id
            }
            None => t.espaco_id,
        };
        if let Some(nome) = self.espaco_arquivado(espaco)? {
            return Err(tfa("erro.desarquive_o_espaco", &[&nome]));
        }
        t.arquivado = false;
        self.banco.editar_timer(&t).map_err(texto)?;
        self.reesconder(agora)
    }

    /// Desarquiva o grupo (os timers dele voltam, menos os arquivados um a um).
    pub fn desarquivar_grupo(&mut self, id: i64, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_banco()?;
        let mut g = self.grupo_qualquer(id)?;
        if !g.arquivado {
            return Err(ta("erro.nao_arquivado").into());
        }
        if let Some(nome) = self.espaco_arquivado(g.espaco_id)? {
            return Err(tfa("erro.desarquive_o_espaco", &[&nome]));
        }
        g.arquivado = false;
        self.banco.editar_grupo(&g).map_err(texto)?;
        self.reesconder(agora)
    }

    /// Ajustes → Arquivados.
    pub fn arquivados(&self, agora: Ms) -> Result<ArquivadosDto, String> {
        let (corte, fuso) = (self.motor.cfg.corte_min, self.motor.cfg.fuso_min);
        let tempo = self.banco.tempo_guardado(agora, corte, fuso).map_err(texto)?;
        let total = |id: i64| tempo.get(&id).map_or(0, |t| t.total_ms);
        let espacos_arq: BTreeSet<i64> =
            self.banco.espacos().map_err(texto)?.into_iter().filter(|e| e.arquivado).map(|e| e.id).collect();
        let grupos = self.banco.grupos_com(true).map_err(texto)?;
        let timers = self.banco.timers_com(true).map_err(texto)?;
        let dto = |t: &nucleo::modelo::Timer| {
            let g = t.grupo_id.and_then(|gid| grupos.iter().find(|g| g.id == gid));
            TimerArquivadoDto {
                id: t.id,
                nome: t.nome.clone(),
                cor: t.cor.clone(),
                arquivado: t.arquivado,
                total_ms: total(t.id),
                grupo_nome: g.filter(|g| !g.arquivado).map(|g| g.nome.clone()),
                espaco_id: g.map_or(t.espaco_id, |g| g.espaco_id),
            }
        };
        let grupos_dto: Vec<GrupoArquivadoDto> = grupos
            .iter()
            .filter(|g| g.arquivado && !espacos_arq.contains(&g.espaco_id))
            .map(|g| {
                let meus: Vec<TimerArquivadoDto> = timers.iter().filter(|t| t.grupo_id == Some(g.id)).map(dto).collect();
                GrupoArquivadoDto {
                    id: g.id,
                    nome: g.nome.clone(),
                    cor: g.cor.clone(),
                    espaco_id: g.espaco_id,
                    total_ms: meus.iter().map(|t| t.total_ms).sum(),
                    timers: meus,
                }
            })
            .collect();
        let timers_dto: Vec<TimerArquivadoDto> = timers
            .iter()
            .filter(|t| t.arquivado)
            .map(dto)
            .filter(|t| !espacos_arq.contains(&t.espaco_id))
            // os de um grupo arquivado aparecem dentro dele
            .filter(|t| timers.iter().find(|x| x.id == t.id).and_then(|x| x.grupo_id).is_none_or(|gid| {
                grupos.iter().any(|g| g.id == gid && !g.arquivado)
            }))
            .collect();
        Ok(ArquivadosDto { grupos: grupos_dto, timers: timers_dto })
    }

    /// Apaga os timers `ids` (e o grupo, se vier): o que roda neles fecha antes, as conversas e as
    /// regras ficam sem timer. Sem tempo guardado, devolve o `Apagado` (o "Desfazer"); com tempo,
    /// só com o `nome` certo e depois de um backup (`dir`), e sem "Desfazer".
    #[allow(clippy::too_many_arguments)]
    fn apagar(
        &mut self,
        ids: &[i64],
        grupo: Option<i64>,
        nome_certo: &str,
        nome: Option<&str>,
        dir: &Path,
        local: NaiveDateTime,
        agora: Ms,
    ) -> Result<Option<Apagado>, String> {
        self.checar_banco()?;
        let uso = self.uso(ids, None, agora)?;
        let com_horas = uso.total_ms > 0 || uso.rodando;
        if com_horas {
            if !nome.is_some_and(|n| nome_confere(n, nome_certo)) {
                return Err(ta(if nome.is_some() { "erro.nome_nao_confere" } else { "erro.tem_horas" }).into());
            }
            self.backup_agora(dir, local).map_err(|e| tfa("erro.backup_antes_de_apagar", &[&e]))?;
        }
        // as conversas ligadas, antes de o motor soltá-las (o "Desfazer" religa; revisão I2)
        let sessoes = self.banco.sessoes_dos_timers(ids).map_err(texto)?;
        // o motor larga os timers (fecha o que roda neles e solta as sessões) antes de sumirem
        let mut esc = self.motor.escondidos.clone();
        esc.extend(ids.iter().copied());
        let mud = self.motor.esconder(esc, agora);
        let feito = self.aplicar(&mud).and_then(|_| {
            let mut a = self.banco.apagar_timers(ids).map_err(texto)?;
            if let Some(g) = grupo {
                a.linhas.insert(0, self.banco.apagar_grupo_vazio(g).map_err(texto)?);
            }
            a.sessoes = sessoes;
            Ok(a)
        });
        let a = feito.map(|a| (!com_horas).then_some(a));
        // no erro, o motor volta a ver o timer (ele segue no banco; revisão M3)
        self.depois_de_arquivar(a.as_ref().map(|_| ()).map_err(Clone::clone), agora)?;
        a
    }

    /// Apaga o timer (o "Apagar…" do ⋯ e dos Arquivados).
    pub fn apagar_timer(&mut self, id: i64, nome: Option<&str>, dir: &Path, local: NaiveDateTime, agora: Ms) -> Result<Option<Apagado>, String> {
        let t = self.timer_qualquer(id)?;
        self.apagar(&[id], None, &t.nome, nome, dir, local, agora)
    }

    /// Apaga o grupo: "Só o grupo" (`com_timers` falso: os timers vão para o "Sem grupo", com as
    /// horas, e sempre há "Desfazer") ou o grupo e os timers dele, como o `apagar_timer`.
    pub fn apagar_grupo(
        &mut self,
        id: i64,
        com_timers: bool,
        nome: Option<&str>,
        dir: &Path,
        local: NaiveDateTime,
        agora: Ms,
    ) -> Result<Option<Apagado>, String> {
        let g = self.grupo_qualquer(id)?;
        if !com_timers {
            self.checar_banco()?;
            let a = self.banco.apagar_so_o_grupo(id).map_err(texto)?;
            let m = self.reesconder(agora)?;
            self.aplicar(&m)?;
            return Ok(Some(a));
        }
        let ids = self.timers_do_grupo(id)?;
        self.apagar(&ids, Some(id), &g.nome, nome, dir, local, agora)
    }

    /// O "Desfazer" do apagar: tudo volta, com os mesmos ids; as sessões em memória que seguem sem
    /// timer voltam a ele.
    pub fn desfazer_apagar(&mut self, a: &Apagado, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_banco()?;
        // um id reusado (UNIQUE) tem texto próprio; outro erro do banco vai como veio (re-revisão R2)
        self.banco.por_de_volta(a).map_err(|e| {
            if nucleo::banco::lugar_ocupado(&e) {
                ta("erro.desfazer_ocupado").to_string()
            } else {
                tfa("erro.nao_da_desfazer", &[&e.to_string()])
            }
        })?;
        let mut mud = self.reesconder(agora)?;
        for (sessao, timer, ligacao) in &a.sessoes {
            mud.extend(self.motor.religar(sessao, *timer, ligacao.as_deref().and_then(Ligacao::de_str), agora));
        }
        Ok(mud)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::estado_app::testes::{ev, h, nucleo};
    use nucleo::modelo::{MotivoFim, Origem};

    fn local() -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap().and_hms_opt(12, 0, 0).unwrap()
    }
    fn com_trecho(n: &mut Nucleo, timer: i64, de: Ms, ate: Ms) {
        n.aplicar(&[
            Mudanca::Abriu { timer_id: Some(timer), sessao_id: None, origem: Origem::Manual, inicio: de, atividade_id: 1 },
            Mudanca::Fechou { timer_id: Some(timer), sessao_id: None, origem: Origem::Manual, fim: ate, motivo: MotivoFim::Pausa },
        ])
        .unwrap();
    }
    fn evento(n: &mut Nucleo, e: fila::EventoHook) {
        let m = n.motor.evento(&e);
        n.aplicar(&m).unwrap();
    }

    #[test]
    fn nome_confere_sem_maiusculas_nem_espacos_das_pontas() {
        assert!(nome_confere(" sprint 12 ", "Sprint 12"));
        assert!(!nome_confere("Sprint 1", "Sprint 12"));
        assert!(!nome_confere("  ", ""));
    }

    /// Arquivar com "Mover para": a conversa trabalhando segue no destino sem buraco, a regra e a
    /// sessão do banco vão junto; a conversa nova na pasta conta no destino.
    #[test]
    fn arquivar_movendo_as_conversas_e_a_regra() {
        let mut n = nucleo(h(8, 0));
        let sprint = n.banco.criar_timer(None, "Sprint 12", "#fff").unwrap();
        let outro = n.banco.criar_timer(None, "Sprint 13", "#fff").unwrap();
        n.banco.criar_regra_pasta(r"D:\proj", sprint).unwrap();
        n.reesconder(h(8, 0)).unwrap();
        evento(&mut n, ev(h(9, 0), "UserPromptSubmit", "s1", r"D:\proj"));
        let uso = n.uso_do_timer(sprint, h(9, 10)).unwrap();
        assert_eq!((uso.conversas, uso.regras.clone(), uso.rodando), (1, vec![r"D:\proj".to_string()], true));
        // como o `agir`: a função e depois o que ela devolve (revisão I1: nada, já foi aplicado)
        let mud = n.arquivar_timer(sprint, Some(outro), h(9, 10)).unwrap();
        assert!(mud.is_empty());
        n.aplicar(&mud).unwrap();
        let abertos = n.banco.abertos().unwrap();
        assert_eq!(abertos.len(), 1, "um trecho aberto só, no banco e no motor: {abertos:?}");
        assert_eq!((abertos[0].timer_id, abertos[0].inicio), (Some(outro), h(9, 10)));
        assert_eq!(n.motor.abertos.len(), 1);
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(outro));
        assert_eq!(n.banco.regras_pasta().unwrap()[0].timer_id, outro);
        assert!(n.motor.escondidos.contains(&sprint));
        // a conversa nova na pasta vai para o destino
        evento(&mut n, ev(h(9, 20), "UserPromptSubmit", "s2", r"D:\proj\api"));
        assert_eq!(n.motor.sessoes["s2"].timer_id, Some(outro));
        // o destino não pode ser o próprio nem um escondido
        assert!(n.arquivar_timer(outro, Some(sprint), h(9, 30)).is_err());
    }

    /// "Não contar mais": a regra sai, a conversa fica sem timer e pergunta; nada vai ao arquivado.
    #[test]
    fn arquivar_sem_contar_mais_e_desarquivar() {
        let mut n = nucleo(h(8, 0));
        let sprint = n.banco.criar_timer(None, "Sprint 12", "#fff").unwrap();
        n.banco.criar_regra_pasta(r"D:\proj", sprint).unwrap();
        n.reesconder(h(8, 0)).unwrap();
        evento(&mut n, ev(h(9, 0), "UserPromptSubmit", "s1", r"D:\proj"));
        let mud = n.arquivar_timer(sprint, None, h(9, 10)).unwrap();
        n.aplicar(&mud).unwrap();
        assert_eq!(n.banco.abertos().unwrap().len(), 1, "revisão I1: o aberto sem timer uma vez só");
        assert!(n.banco.regras_pasta().unwrap().is_empty());
        assert_eq!(n.motor.sessoes["s1"].timer_id, None);
        assert!(!n.motor.abertos.iter().any(|a| a.timer_id == Some(sprint)));
        assert!(n.motor.perguntas.iter().any(|(_, p)| matches!(p, nucleo::modelo::Pergunta::SessaoNova { .. })));
        // a bandeja não conta um rodando escondido; o snapshot não mostra, mas soma no dia
        let s = n.snapshot(h(9, 15)).unwrap();
        assert!(s.escondidos_hoje.iter().any(|e| e.id == sprint));
        // o ▶ num arquivado recusa
        assert!(n.checar_timer(sprint).is_err());
        assert!(n.arquivar_timer(sprint, None, h(9, 16)).is_err(), "já arquivado");
        // desarquivar: volta à tela, a conversa não volta sozinha
        n.desarquivar_timer(sprint, h(9, 20)).unwrap();
        assert!(!n.motor.escondidos.contains(&sprint));
        assert_eq!(n.motor.sessoes["s1"].timer_id, None);
        assert!(n.desarquivar_timer(sprint, h(9, 21)).is_err());
    }

    #[test]
    fn arquivar_grupo_e_os_arquivados() {
        let mut n = nucleo(h(8, 0));
        let g = n.banco.criar_grupo("Casa", "#fff").unwrap();
        let obra = n.banco.criar_timer(Some(g), "Obra", "#fff").unwrap();
        let jardim = n.banco.criar_timer(Some(g), "Jardim", "#fff").unwrap();
        let solto = n.banco.criar_timer(None, "Velho", "#fff").unwrap();
        com_trecho(&mut n, obra, h(8, 0), h(9, 0));
        n.motor.cfg.um_play_por_vez = false;
        let m = n.motor.play(jardim, h(9, 0));
        n.aplicar(&m).unwrap();
        let mud = n.arquivar_grupo(g, None, h(9, 30)).unwrap();
        n.aplicar(&mud).unwrap();
        assert!(n.motor.abertos.is_empty(), "o ▶ do grupo arquivado fecha");
        n.arquivar_timer(solto, None, h(9, 31)).unwrap();
        let a = n.arquivados(h(10, 0)).unwrap();
        assert_eq!(a.grupos.len(), 1);
        assert_eq!(a.grupos[0].timers.len(), 2);
        assert_eq!(a.grupos[0].total_ms, 90 * 60_000);
        assert_eq!(a.timers.iter().map(|t| t.id).collect::<Vec<_>>(), vec![solto]);
        // o timer arquivado dentro do grupo arquivado: desarquive o grupo antes
        n.arquivar_grupo(g, None, h(10, 1)).unwrap_err();
        let mut t = n.banco.timers().unwrap().into_iter().find(|t| t.id == obra).unwrap();
        t.arquivado = true;
        n.banco.editar_timer(&t).unwrap();
        assert_eq!(n.desarquivar_timer(obra, h(10, 2)).unwrap_err(), "Desarquive antes o grupo Casa");
        n.desarquivar_grupo(g, h(10, 3)).unwrap();
        assert!(n.motor.escondidos.contains(&obra) && !n.motor.escondidos.contains(&jardim));
        assert_eq!(n.arquivados(h(10, 4)).unwrap().timers.len(), 2, "a Obra aparece sozinha, com o grupo à vista");
    }

    /// Revisão C1: responder "Em qual timer contar?" depois do "Não contar mais" leva só o que veio
    /// depois do arquivar; as horas de antes ficam no arquivado.
    #[test]
    fn responder_depois_de_arquivar_nao_leva_o_historico() {
        let mut n = nucleo(h(8, 0));
        let sprint = n.banco.criar_timer(None, "Sprint 12", "#fff").unwrap();
        let outro = n.banco.criar_timer(None, "Sprint 13", "#fff").unwrap();
        n.banco.criar_regra_pasta(r"D:\proj", sprint).unwrap();
        n.reesconder(h(8, 0)).unwrap();
        evento(&mut n, ev(h(9, 0), "UserPromptSubmit", "s1", r"D:\proj"));
        let antes = n.banco.tempo_guardado(h(9, 10), 0, -180).unwrap()[&sprint].total_ms;
        assert_eq!(antes, 10 * 60_000);
        n.arquivar_timer(sprint, None, h(9, 10)).unwrap();
        let (id, _) = n.motor.perguntas.iter().find(|(_, p)| matches!(p, nucleo::modelo::Pergunta::SessaoNova { .. })).cloned().unwrap();
        let mud = n.responder(id, &format!("ligar:{outro}"), h(9, 20)).unwrap();
        n.aplicar(&mud).unwrap();
        let g = n.banco.tempo_guardado(h(9, 20), 0, -180).unwrap();
        assert_eq!(g[&sprint].total_ms, antes, "o arquivado guarda as horas dele");
        assert_eq!(g[&outro].total_ms, 10 * 60_000, "o novo, só desde o arquivar");
    }

    /// Revisão I2: o Desfazer do apagar religa a conversa que estava em memória (e tira a pergunta).
    #[test]
    fn desfazer_apagar_religa_a_conversa_em_memoria() {
        let d = tempfile::tempdir().unwrap();
        let mut n = nucleo(h(8, 0));
        let teste = n.banco.criar_timer(None, "Teste", "#fff").unwrap();
        n.banco.criar_regra_pasta(r"D:\teste", teste).unwrap();
        n.reesconder(h(8, 0)).unwrap();
        evento(&mut n, ev(h(9, 0), "SessionStart", "s1", r"D:\teste"));
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(teste));
        let a = n.apagar_timer(teste, None, d.path(), local(), h(9, 5)).unwrap().expect("sem horas");
        assert_eq!(a.sessoes.len(), 1);
        assert_eq!(n.motor.sessoes["s1"].timer_id, None);
        let mud = n.desfazer_apagar(&a, h(9, 6)).unwrap();
        n.aplicar(&mud).unwrap();
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(teste));
        assert_eq!(n.banco.sessoes_dos_timers(&[teste]).unwrap().len(), 1);
        assert!(n.motor.perguntas.is_empty());
        // um id novo no lugar: a recusa com texto próprio
        let a = n.apagar_timer(teste, None, d.path(), local(), h(9, 7)).unwrap().unwrap();
        let novo = n.banco.criar_timer(None, "Novo", "#fff").unwrap();
        assert_eq!(novo, teste, "o SQLite reusa o id");
        assert_eq!(n.desfazer_apagar(&a, h(9, 8)).unwrap_err(), ta("erro.desfazer_ocupado"));
    }

    /// Sem tempo: apaga direto e o Desfazer põe tudo de volta; com tempo: o nome e o backup.
    #[test]
    fn apagar_sem_e_com_horas() {
        let d = tempfile::tempdir().unwrap();
        let mut n = nucleo(h(8, 0));
        let vazio = n.banco.criar_timer(None, "Teste", "#fff").unwrap();
        let cheio = n.banco.criar_timer(None, "Sprint 12", "#fff").unwrap();
        let fica = n.banco.criar_timer(None, "Fica", "#fff").unwrap();
        n.banco.criar_regra_pasta(r"D:\teste", vazio).unwrap();
        com_trecho(&mut n, cheio, h(8, 0), h(9, 0));
        com_trecho(&mut n, fica, h(8, 0), h(9, 0));
        n.reesconder(h(8, 0)).unwrap();
        let a = n.apagar_timer(vazio, None, d.path(), local(), h(10, 0)).unwrap().expect("com Desfazer");
        assert!(!n.banco.timers().unwrap().iter().any(|t| t.id == vazio));
        assert!(n.banco.regras_pasta().unwrap().is_empty());
        n.desfazer_apagar(&a, h(10, 1)).unwrap();
        assert!(n.banco.timers().unwrap().iter().any(|t| t.id == vazio));
        assert_eq!(n.motor.regras_pasta.len(), 1, "a regra volta ao motor");
        // com horas: sem o nome, ou com o nome errado, recusa e nada muda
        assert_eq!(n.apagar_timer(cheio, None, d.path(), local(), h(10, 2)).unwrap_err(), ta("erro.tem_horas"));
        assert_eq!(n.apagar_timer(cheio, Some("sprint"), d.path(), local(), h(10, 2)).unwrap_err(), ta("erro.nome_nao_confere"));
        assert!(nucleo::dados::listar(d.path()).is_empty());
        // com o nome: backup antes, as horas somem do dia, o outro fica
        assert!(n.apagar_timer(cheio, Some(" sprint 12"), d.path(), local(), h(10, 3)).unwrap().is_none());
        assert_eq!(nucleo::dados::listar(d.path()).len(), 1);
        let s = n.snapshot(h(10, 4)).unwrap();
        assert_eq!(s.grupos.iter().flat_map(|g| &g.timers).map(|t| t.id).collect::<Vec<_>>(), vec![vazio, fica]);
        assert!(s.escondidos_hoje.is_empty());
    }

    #[test]
    fn apagar_grupo_so_o_grupo_ou_com_os_timers() {
        let d = tempfile::tempdir().unwrap();
        let mut n = nucleo(h(8, 0));
        let g = n.banco.criar_grupo("Casa", "#fff").unwrap();
        let obra = n.banco.criar_timer(Some(g), "Obra", "#fff").unwrap();
        com_trecho(&mut n, obra, h(8, 0), h(9, 0));
        let a = n.apagar_grupo(g, false, None, d.path(), local(), h(10, 0)).unwrap().unwrap();
        assert_eq!(n.banco.timers().unwrap()[0].grupo_id, None, "a Obra vai para o Sem grupo, com as horas");
        n.desfazer_apagar(&a, h(10, 1)).unwrap();
        assert_eq!(n.banco.timers().unwrap()[0].grupo_id, Some(g));
        // o grupo e os timers, com horas: o nome do grupo
        assert!(n.apagar_grupo(g, true, Some("Obra"), d.path(), local(), h(10, 2)).is_err());
        assert!(n.apagar_grupo(g, true, Some("casa"), d.path(), local(), h(10, 3)).unwrap().is_none());
        assert!(n.banco.timers_com(true).unwrap().is_empty());
        assert!(n.banco.grupos_com(true).unwrap().is_empty());
        // um grupo vazio apaga com Desfazer
        let g2 = n.banco.criar_grupo("Vazio", "#fff").unwrap();
        let a = n.apagar_grupo(g2, true, None, d.path(), local(), h(10, 4)).unwrap().unwrap();
        assert!(n.banco.grupos_com(true).unwrap().is_empty());
        n.desfazer_apagar(&a, h(10, 5)).unwrap();
        assert_eq!(n.banco.grupos().unwrap()[0].id, g2);
    }

    /// Arquivar o espaço esconde os timers dele: o ▶ fecha e a conversa ligada fica sem timer.
    #[test]
    fn espaco_arquivado_nao_conta() {
        let mut n = nucleo(h(8, 0));
        let novo = n.criar_espaco("Pessoal", "#0af").unwrap();
        let t = n.banco.criar_timer_no(None, novo, "Leitura", "#fff", h(8, 0), -180).unwrap();
        n.banco.criar_regra_pasta(r"D:\livros", t).unwrap();
        n.reesconder(h(8, 0)).unwrap();
        evento(&mut n, ev(h(9, 0), "UserPromptSubmit", "s1", r"D:\livros"));
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(t));
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == novo).unwrap();
        e.arquivado = true;
        let mud = n.editar_espaco(&e, h(9, 10)).unwrap();
        n.aplicar(&mud).unwrap();
        assert!(n.motor.escondido(Some(t)));
        assert_eq!(n.motor.sessoes["s1"].timer_id, None);
        assert!(!n.motor.abertos.iter().any(|a| a.timer_id == Some(t)));
        // a regra fica no banco (inerte) e volta a valer ao desarquivar o espaço
        evento(&mut n, ev(h(9, 20), "UserPromptSubmit", "s2", r"D:\livros"));
        assert_eq!(n.motor.sessoes["s2"].timer_id, None);
        e.arquivado = false;
        let mud = n.editar_espaco(&e, h(9, 30)).unwrap();
        n.aplicar(&mud).unwrap();
        evento(&mut n, ev(h(9, 40), "UserPromptSubmit", "s3", r"D:\livros"));
        assert_eq!(n.motor.sessoes["s3"].timer_id, Some(t));
    }

    /// Ao abrir o app, o que ficou ligado a um arquivado de antes (versões antigas) é solto.
    #[test]
    fn ao_abrir_solta_o_que_estava_no_arquivado() {
        let b = nucleo::banco::Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Velho", "#fff").unwrap();
        b.criar_regra_pasta(r"D:\proj", t).unwrap();
        let mut x = b.timers().unwrap().remove(0);
        x.arquivado = true;
        b.editar_timer(&x).unwrap();
        let n = Nucleo::abrir(b, h(9, 0), -180, chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap()).unwrap();
        assert!(n.motor.escondidos.contains(&t));
        assert_eq!(n.motor.regra_viva(r"D:\proj"), None);
    }
}
