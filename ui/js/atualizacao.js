// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Atualizar pelo app (rodada 25): o ícone no cabeçalho quando há versão nova, o diálogo de
// atualizar (as notas, o progresso do download) e a linha do "Verificar atualizações" nos
// Ajustes → Sobre. Quem busca e instala é o núcleo (`atualizacao_procurar`, `atualizacao_instalar`);
// a página só mostra. A novidade é `{ versao, notas }`, ou nula.
import { chamar, ouvir } from './api.js';
import { esc } from './componentes.js';
import { abrirDialogo, textoDoErro } from './editar.js';
import { dataDoBackup, tamanho } from './formato.js';
import { PERFIL_TESTE } from './nome.js';
import { TEXTOS } from './textos.js';

/**
 * O ícone do cabeçalho: só com versão nova, com a dica "Atualizar para 0.4.1". No perfil de teste
 * não aparece (revisão da rodada 25): ali a instalação fica desligada, porque o exe é o mesmo do
 * uso; o "Verificar" dos Ajustes mostra o resultado.
 */
export function estadoDoIcone(novidade, teste = PERFIL_TESTE) {
  if (!novidade?.versao || teste) return { visivel: false, dica: '' };
  return { visivel: true, dica: TEXTOS.atualizarPara(novidade.versao) };
}

/** "Versão 0.4.0 · alpha" (o canal vem do núcleo: `nucleo::CANAL`). */
export const textoDaVersao = (versao, canal) => (canal ? TEXTOS.versaoComCanal(versao, canal) : versao);

/** O corpo do diálogo: o que acontece, as notas da versão (se houver) e o lugar do progresso. */
export function corpoAtualizar(novidade) {
  const notas = String(novidade?.notas ?? '').trim();
  return `<p class="dlg-p">${esc(TEXTOS.atualizarTexto)}</p>
${notas ? `<div class="atu-notas"><b>${esc(TEXTOS.notasDaVersao)}</b><div class="atu-notas-tx">${esc(notas)}</div></div>` : ''}
<div class="atu-prog" role="status" hidden><div class="atu-barra"><i></i></div><small></small></div>`;
}

/**
 * O progresso do download: `{ pct, texto }`. Com o tamanho total, a porcentagem ("Baixando… 45%");
 * sem ele, o que já veio ("Baixando… 3.2 MB"); no fim, "Instalando…".
 */
export function progresso(baixado, total) {
  if (total && baixado >= total) return { pct: 100, texto: TEXTOS.instalando };
  if (total) {
    const pct = Math.max(0, Math.min(99, Math.floor((baixado / total) * 100)));
    return { pct, texto: TEXTOS.baixando(`${pct}%`) };
  }
  return { pct: null, texto: TEXTOS.baixando(baixado ? tamanho(baixado) : '') };
}

function mostrarProgresso(form, baixado, total) {
  const el = form?.querySelector('.atu-prog');
  if (!el) return;
  const p = progresso(baixado, total);
  el.hidden = false;
  el.querySelector('.atu-barra i').style.width = p.pct === null ? '' : `${p.pct}%`;
  el.classList.toggle('sem-total', p.pct === null);
  el.querySelector('small').textContent = p.texto;
}

/**
 * O diálogo "Versão 0.4.1 disponível": "Atualizar e reiniciar" e "Depois". Durante o download, o
 * progresso; no fim, o instalador passivo roda e o app reabre. Erro (rede, assinatura): a mensagem
 * no diálogo, e o app segue como está.
 */
export function abrirAtualizar(novidade, teste = PERFIL_TESTE) {
  if (!novidade?.versao || teste) return;
  let parar = null;
  abrirDialogo({
    titulo: esc(TEXTOS.atualizarTitulo(novidade.versao)),
    corpo: corpoAtualizar(novidade),
    enviar: TEXTOS.atualizarEReiniciar,
    cancelar: TEXTOS.depois,
    classe: 'dlg-atualizar',
    aoMontar(form) {
      ouvir('atualizacao-progresso', (p) => mostrarProgresso(form, p?.baixado ?? 0, p?.total ?? null))
        .then((f) => (parar = f))
        .catch(() => {});
    },
    async aoEnviar(form) {
      mostrarProgresso(form, 0, null);
      try {
        await chamar('atualizacao_instalar');
      } catch (e) {
        const el = form.querySelector('.atu-prog');
        if (el) el.hidden = true;
        throw TEXTOS.naoDeuAtualizar(textoDoErro(e));
      }
    },
    aoFechar() {
      parar?.();
    },
  });
}

// ---- Ajustes → Sobre ----

/**
 * A linha do "Verificar atualizações": `estado` é nulo (nada ainda), "procurando", "recente",
 * "disponivel" (com a `novidade`, e o "Atualizar" que abre o diálogo) ou "erro". No perfil de
 * teste, a versão achada vem sem o "Atualizar", com a frase de que a instalação fica desligada.
 */
export function linhaDaVerificacao(estado, novidade = null, teste = PERFIL_TESTE) {
  switch (estado) {
    case 'procurando':
      return `<span class="aj-atu-res procurando">${esc(TEXTOS.atuProcurando)}</span>`;
    case 'recente':
      return `<span class="aj-atu-res ok">${esc(TEXTOS.atuRecente)}</span>`;
    case 'disponivel': {
      const versao = esc(TEXTOS.atuDisponivel(novidade?.versao ?? ''));
      if (teste) return `<span class="aj-atu-res nova">${versao} · ${esc(TEXTOS.atuSoNoUso)}</span>`;
      return `<span class="aj-atu-res nova">${versao} · <button type="button" class="aj-link" data-aj="atu-instalar">${esc(TEXTOS.atuAtualizar)}</button></span>`;
    }
    case 'erro':
      return `<span class="aj-atu-res erro">${esc(TEXTOS.atuErro)}</span>`;
    default:
      return '<span class="aj-atu-res"></span>';
  }
}

/** O instante (ms) como data e hora locais, no formato das datas dos backups. */
export function dataHoraLocal(ms) {
  const d = new Date(ms);
  const dois = (n) => String(n).padStart(2, '0');
  return dataDoBackup(`${d.getFullYear()}-${dois(d.getMonth() + 1)}-${dois(d.getDate())}T${dois(d.getHours())}:${dois(d.getMinutes())}`);
}

/** "Última verificação: 20/11/2026 15:04" (ou "Ainda não verificou"). */
export const textoDaUltima = (ms) => (ms ? TEXTOS.atuUltima(dataHoraLocal(ms)) : TEXTOS.atuNunca);

/**
 * O bloco de atualização do Sobre: o botão com o resultado na linha, a caixa da busca automática
 * (com a dica: uma vez por dia; procurar e baixar atualizações é o único uso da internet do app)
 * e a data da última busca.
 */
export function htmlAtualizacao({ ligada, ultima, estado = null, novidade = null, app = '' }) {
  return `<div class="aj-atu" data-bloco="atualizacao">
<div class="aj-atu-linha"><button type="button" class="bt" data-aj="atu-verificar"${estado === 'procurando' ? ' disabled' : ''}>${esc(TEXTOS.verificarAtualizacoes)}</button><span data-atu="resultado" role="status">${linhaDaVerificacao(estado, novidade)}</span></div>
<label class="chk">
  <input type="checkbox" name="atualizar_auto"${ligada ? ' checked' : ''}><span>${esc(TEXTOS.atualizarAuto)}<small>${esc(TEXTOS.atualizarAutoDica(app))}</small></span>
</label>
<small class="aj-atu-data" data-atu="ultima">${esc(textoDaUltima(ultima))}</small>
</div>`;
}
