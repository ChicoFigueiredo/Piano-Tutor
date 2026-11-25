-- Biblioteca comum do Curso (exemplo): funções que várias Aulas reaproveitam.
-- Uso, no Roteiro de uma Aula:  local basico = require("basico")
-- O nome do módulo é o nome do arquivo sem a extensão. Só existem módulos desta pasta
-- (roteiros-aula/comum); não há acesso a arquivos, rede nem relógio.

local M = {}

-- Elogios curtos, para o professor falar sem repetir sempre a mesma frase.
local ELOGIOS = {
	"Muito bem!",
	"Isso mesmo!",
	"Perfeito, sem errar nenhuma nota!",
}

-- Devolve um elogio. `sorteio` é qualquer número (por exemplo, um contador da Aula),
-- porque o Roteiro não tem gerador de números aleatórios: assim o resultado é previsível.
function M.elogio(sorteio)
	local i = (math.floor(sorteio or 0) % #ELOGIOS) + 1
	return ELOGIOS[i]
end

-- Conta quantas vezes o aluno terminou um trecho com erro, por trecho.
-- Usa os contadores da API (`aula.contar`), que sobrevivem entre os callbacks.
function M.contar_tentativa(t)
	return aula.contar("tentativas:" .. t.id)
end

function M.tentativas(t)
	return aula.contador("tentativas:" .. t.id)
end

-- Zera as tentativas de um trecho (por exemplo, ao acertar).
function M.zerar_tentativas(t)
	aula.zerar("tentativas:" .. t.id)
end

return M
