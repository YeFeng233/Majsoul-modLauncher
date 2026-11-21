local M, mgr = MJLocalTrial, GameMgr.Inst
M.guard_ready = true
local server = { characters = { { charid = 200001, skin = 400101 } }, skins = { 400101 },
  main_character_id = 200001, character_sort = { 200001 }, other_character_sort = {}, hidden_characters = {} }
GameMgr.makeCharacterInfo(mgr, server)
assert(M.character_sort[1] == 200002 and #M.character_sort == 1,
  'favorite lost across full runtime restart')
assert(#mgr.characterSort == 1 and mgr.characterSort[1] == 200002,
  'native client did not receive restored favorite')
assert(#mgr.otherCharacterSort == 1 and mgr.otherCharacterSort[1] == 200001)
assert(#server.character_sort == 1 and server.character_sort[1] == 200001)
assert(M.choices.character_id == 200002 and M.skins[200002] == 400202,
  'favorites persistence broke the existing appearance profile')
-- Persist an explicitly empty favorite list, then reload the account again.
LobbyNetMgr.SendRequest('Lobby', 'updateCharacterSort',
  { sort = {}, other_sort = { 200001, 200002 }, hidden_characters = {} }, nil)
M.account_id = nil
GameMgr.makeCharacterInfo(mgr, server)
assert(#mgr.characterSort == 0, 'last removed star returned after restoring profile')
