-- Roteiro de Aula (exemplo): Aula 14 «Cinco notas», do módulo S03.
-- ADR-0012. Este arquivo vive em  <pasta do Curso>/roteiros-aula/s03-a14.luau
-- e usa a biblioteca comum em  <pasta do Curso>/roteiros-aula/comum/*.luau.
--
-- A Aula tem um único trecho, «cinco-notas». O fluxo:
--   1. Ao entrar no primeiro trecho, o modo de acompanhamento é «espera»
--      (o vídeo para em cada nota até o aluno tocar a certa).
--   2. Ao terminar o trecho COM erros: pede para repetir, até 3 vezes.
--      Na 4ª vez, em vez de repetir, sugere o modo «segue» e deixa o aluno continuar.
--   3. Ao terminar SEM erros: o professor elogia em voz alta e aparece uma
--      mensagem para seguir para a próxima Aula.
--
-- Os callbacks (ao_abrir, ao_entrar_trecho, ao_fim_trecho, ao_nota) são opcionais; os que
-- não aparecem aqui simplesmente não fazem nada. As funções de `aula.*` só ENFILEIRAM
-- pedidos: quem decide executá-los é o player.

local basico = require("basico")

local MAX_REPETICOES = 3

-- Variáveis locais do arquivo sobrevivem entre os callbacks.
local elogios_dados = 0

function ao_entrar_trecho(t)
	-- `t.indice` conta de 1. Só o primeiro trecho força o modo espera.
	if t.indice == 1 then
		aula.modo("espera")
	end
end

function ao_fim_trecho(t, r)
	if r.erros > 0 then
		local feitas = basico.tentativas(t)
		if feitas < MAX_REPETICOES then
			basico.contar_tentativa(t)
			aula.mensagem("Quase lá! Vamos repetir (" .. (feitas + 1) .. " de " .. MAX_REPETICOES .. ").")
			aula.repetir()
		else
			-- Já repetiu o suficiente: sugere o modo que não para em cada nota.
			basico.zerar_tentativas(t)
			aula.modo("segue")
			aula.mensagem("Tudo bem errar. Vamos tocar junto com o vídeo, sem parar em cada nota.")
		end
	else
		basico.zerar_tentativas(t)
		aula.dizer(basico.elogio(elogios_dados))
		elogios_dados = elogios_dados + 1
		aula.mensagem("Você tocou as cinco notas. Siga para a próxima Aula!")
	end
end
