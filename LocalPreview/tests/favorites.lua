local M, mgr = MJLocalTrial, GameMgr.Inst
mgr.account_id = 123
local key = 'MJLocalPreview.' .. 'characterSort.v1.' .. tostring(mgr.account_id)
PLAYER_PREFS[key] = nil
M.account_id = nil
M.account(mgr)
local server = { characters = { { charid = 200001, skin = 400101 } }, skins = { 400101 },
  main_character_id = 200001, character_sort = { 200001 }, other_character_sort = {}, hidden_characters = {} }
local make_info, send = GameMgr.makeCharacterInfo, LobbyNetMgr.SendRequest
make_info(mgr, server)
assert(mgr.characterSort[1] == 200001, 'first login must preserve server favorites')
local function save(favorites, others, hidden)
  local request = { sort = favorites, other_sort = others, hidden_characters = hidden or {} }
  local calls, sent_before = 0, #SENT
  send('Lobby', 'updateCharacterSort', request, function(err, res)
    assert(not err and res.error.code == 0)
    assert(PLAYER_PREFS[key], 'favorites must be persisted before success callback')
    calls = calls + 1
  end)
  assert(calls == 1 and #SENT == sent_before, 'favorite request escaped local handling')
  return request
end
local request = save({ 200002, 200001 }, {})
assert(PLAYER_PREFS[key] == '1;200002,200001;;')
request.sort[1] = 999999
assert(M.character_sort[1] == 200002, 'request list must be copied')
make_info(mgr, server)
assert(mgr.characterSort[1] == 200002 and mgr.characterSort[2] == 200001,
  'reconnect must preserve favorite order')
assert(server.character_sort[1] == 200001 and #server.character_sort == 1,
  'server packet was modified')
-- Removing one star, then clearing all stars, must override server favorites.
save({ 200002 }, { 200001 })
make_info(mgr, server)
assert(#mgr.characterSort == 1 and mgr.characterSort[1] == 200002)
save({}, { 200002, 200001 })
M.account_id = nil; M.account(mgr); make_info(mgr, server)
assert(#mgr.characterSort == 0 and mgr.otherCharacterSort[1] == 200002,
  'cleared favorites must not fall back to server favorites')
-- Missing/new characters stay in the ordinary list exactly once.
save({ 200002 }, {})
make_info(mgr, server)
assert(#mgr.otherCharacterSort == 1 and mgr.otherCharacterSort[1] == 200001)
-- Validate IDs, remove duplicates, preserve hidden favorites and list order.
save({ 200002, 200002, 999999 }, { 200002, 200001, 200001 }, { 200002 })
assert(PLAYER_PREFS[key] == '1;200002;200001;200002')
make_info(mgr, server)
assert(#mgr.characterSort == 0 and mgr.hiddenFavorCharacters[1] == 200002)
assert(mgr.hiddenCharacters[1] == 200002)
-- Account isolation and malformed data do not inherit another account's stars.
mgr.account_id = 789; M.account(mgr)
assert(M.character_sort == nil)
make_info(mgr, server)
assert(mgr.characterSort[1] == 200001)
PLAYER_PREFS['MJLocalPreview.characterSort.v1.789'] = 'return os.execute("unexpected")'
M.account_id = nil; M.account(mgr)
assert(M.character_sort == nil)
PLAYER_PREFS['MJLocalPreview.characterSort.v1.789'] = '1;999999,200002,200002;200002,200001;999999'
M.account_id = nil; M.account(mgr)
assert(#M.character_sort == 1 and M.character_sort[1] == 200002)
assert(#M.other_character_sort == 1 and #M.hidden_characters == 0)
mgr.account_id = 123; M.account(mgr)
assert(M.character_sort[1] == 200002 and M.hidden_characters[1] == 200002)
-- Leave a visible favorite for the full runtime-reset check.
save({ 200002 }, { 200001 })
-- Updating the appearance profile must not erase the independent favorites key.
M.choose(200002, 400202, true)
assert(PLAYER_PREFS[key] == '1;200002;200001;')
