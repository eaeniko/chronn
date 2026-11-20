// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Os textos novos da interface (rodada 20: espaços, o "+ Novo" e as atividades no painel), num
// lugar só: a interface em inglês, numa rodada futura, troca esta lista.

export const TEXTOS = {
  // abas de espaço
  todos: 'Todos',
  abasRotulo: 'Espaços',
  espaco: 'Espaço',
  semGrupo: 'Sem grupo',
  grupo: 'Grupo',
  /** Junto ao nome de um grupo ou espaço arquivado num seletor: "Casa (arquivado)". */
  arquivado: 'arquivado',
  /** A dica do total do dia com mais de um espaço: o geral e o de cada espaço. */
  totalGeral: 'Total geral',
  /** "Zerar todos" e "Pausar tudo" de uma aba: "Zerar todos (Pessoal)". */
  zerarTodos: 'Zerar todos',
  pausarTudo: 'Pausar tudo',
  daAba: (texto, espaco) => (espaco ? `${texto} (${espaco})` : texto),
  /** "2 rodando" na linha de baixo do cabeçalho (rodada 21). */
  rodando: (n) => `${n} rodando`,
  /** O texto ao lado do número (compacto, só o número fica; rodada 21). */
  rodandoRotulo: 'rodando',
  // Ajustes → Aparência (rodada 21: o tema saiu do cabeçalho)
  temaClaro: 'Claro',
  temaEscuro: 'Escuro',
  temaSistema: 'Sistema',
  temaRotulo: 'Tema',
  // Ajustes → Painel e a seção da linha do tempo no Painel (rodada 21)
  painelTitulo: 'Painel',
  linhaDoTempo: 'Linha do tempo',
  nadaContadoHoje: 'Nada contado hoje',
  mostrarLinhaNoPainel: 'Mostrar a linha do tempo no Painel',
  mostrarLinhaNoPainelDica: 'A linha de hoje, por timer, no fim do Painel. A tela “Linha do tempo” fica como está.',
  // os menus ⋯ do grupo e do timer, e o mover (rodada 21)
  mais: 'Mais',
  editar: 'Editar',
  moverParaOutroEspaco: 'Mover para outro espaço…',
  moverPara: 'Mover para…',
  ondeEsta: 'aqui',
  /** O aviso depois de mover, com o Desfazer: o espaço só quando ele mudou. */
  movido: (nome, destino, espaco) =>
    espaco
      ? `${nome} foi para ${destino}. As horas dele, também as de dias anteriores, agora contam em ${espaco}.`
      : `${nome} foi para ${destino}.`,
  arquivarGrupoTitulo: (nome) => `Arquivar o grupo ${nome}?`,
  arquivarGrupoTexto: 'O grupo e os timers dele saem das listas. As horas ficam no histórico.',
  zeradosTodosDoEspaco: (espaco) => `Todos os timers de ${espaco} zerados.`,
  dicaZerarTodosDoEspaco: (espaco) =>
    `Zerar todos os timers de ${espaco}: o número de cada um volta a 0:00 agora. Nada é apagado: o total do dia, a linha do tempo e o relatório ficam como estão`,
  // o cabeçalho "opção A" (rodada 22, 0.3.1): as telas num menu, o "+ Novo" e o "Lançar o dia" na
  // linha de cima
  telasRotulo: 'Telas',
  telasDica: 'Trocar de tela',
  telaPainel: 'Painel',
  telaFoco: 'Foco',
  telaCartoes: 'Cartões',
  telaAjustes: 'Ajustes',
  novoRotulo: 'Novo',
  lancarDia: 'Lançar o dia',
  emBreve: 'em breve',
  lancarDiaDica: 'Lançar o dia: disponível numa próxima versão',
  tourTelasTitulo: 'As telas',
  tourTelasTexto: 'Painel, Foco, Cartões e Linha do tempo: troque de tela por este menu.',
  // o "+ Novo"
  novo: '+ Novo',
  novoDica: 'Novo timer, grupo ou espaço (tecla N)',
  novoMenu: 'Novo',
  novoTimer: 'Timer',
  novoGrupo: 'Grupo',
  novoEspaco: 'Espaço',
  grupoVazio: 'Grupo vazio',
  grupoVazioTimer: '+ Timer',
  nenhumTimer: 'Nenhum timer ainda. Use o “+ Novo” no topo.',
  // diálogos
  tituloNovoEspaco: 'Novo espaço',
  tituloEditarEspaco: 'Editar espaço',
  espacoPlaceholder: 'Ex.: Pessoal',
  espacoDoTimerDica: 'O espaço do timer sem grupo (com grupo, vale o do grupo).',
  espacoDoGrupoNovoDica: 'O espaço do grupo novo.',
  // Ajustes → Grupos e regras: os espaços
  espacosTitulo: 'Espaços',
  espacosDica: 'Um espaço junta grupos, como Trabalho e Pessoal. Com dois ou mais, o Painel, os Cartões e a Linha do tempo ganham abas.',
  acrescentarEspaco: '+ Espaço',
  arquivar: 'Arquivar',
  desarquivar: 'Desarquivar',
  apagar: 'Apagar',
  subir: 'Subir',
  descer: 'Descer',
  ultimoEspaco: 'Precisa sobrar pelo menos um espaço',
  apagarSoVazio: 'Mova os grupos para outro espaço antes',
  /** A confirmação de apagar um espaço com itens arquivados (que vão para outro). */
  arquivadosVaoPara: (n, espaco) =>
    n === 1 ? `1 item arquivado vai para ${espaco}.` : `${n} itens arquivados vão para ${espaco}.`,
  apagarEspacoVazio: 'O espaço não tem nada dentro.',
  confirmarApagarEspaco: (nome) => `Apagar o espaço ${nome}?`,
  // editar timer → "Atividades no painel"
  atividadesNoPainel: 'Atividades no painel',
  atividadesNoPainelDica: 'As desmarcadas não aparecem no painel do timer, a não ser com tempo desde o zerar ou rodando.',
  atividadePadraoSempre: 'A atividade padrão do timer aparece sempre',
  // tour
  tourNovoTitulo: '+ Novo',
  tourNovoTexto: 'Cria um timer, um grupo ou um espaço. Também pela tecla N.',
  tourAbasTitulo: 'Os espaços',
  tourAbasTexto: 'Cada aba mostra um espaço (Trabalho, Pessoal…). Em “Todos”, tudo junto. O total do dia e o “Zerar todos” valem para a aba.',
};
