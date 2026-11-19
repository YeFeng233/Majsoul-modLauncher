local M = MJLocalTrial
assert(M and M.guard_ready)
-- Match the client's protobuf shape: public payload fields live in _fields,
-- while pairs(message) sees bookkeeping only. Losing these stalls lobby loading.
do
  local descriptors = { { name = 'activity_list' }, { name = 'title_list' }, { name = 'character_info' } }
  local activity = { activities = {} }
  local payload = { activity_list = activity, title_list = { error = { code = 0 }, title_list = { 600001 } }, character_info = { characters = {} } }
  local wire = setmetatable({ _fields = payload }, { _descriptor = { fields = descriptors },
    __index = function(self, key)
      if key == 'HasField' then return function(obj, name) return obj._fields[name] ~= nil end end
      return self._fields[key]
    end,
    __newindex = function() error('protocol payload must stay unchanged') end })
  local view = M.fetch_info(wire)
  assert(view ~= wire and view.activity_list == activity and view.character_info == payload.character_info)
  assert(#view.title_list.title_list == 2 and #wire.title_list.title_list == 1 and view:HasField('activity_list'))
  assert(view.title_list.error.code == 0, 'fetchInfo.title_list must remain a response message')
  assert(rawget(view, '_fields') == nil, 'copy must not share protocol bookkeeping')
end
assert(UI_Bag.GetCount(305044) == 1 and UI_Bag.GetCount(305001) == 7)
assert(UI_Bag.Inst.getItem[305044] == nil, 'trial must not modify real bag')
local deco = UI_Bag.FindItemByCategory(5, 4)
assert(#deco == 1 and deco[1].item_id == 305044 and deco[1].data.type == 4)
assert(#UI_Bag.FindItemByCategory(5, 4, { 4 }) == 0)
assert(#UI_Bag.FindItemByCategory(1) == 1, 'ordinary inventory lookup must pass through')
local server = { characters = { { charid = 200001, skin = 400101, level = 1, is_upgraded = false } },
  skins = { 400101 }, character_sort = { 200001 }, other_character_sort = {}, main_character_id = 200001 }
-- Invoke a captured original function value, proving source instrumentation survives
-- delegates captured before any runtime wrappers could have been installed.
local captured = GameMgr.makeCharacterInfo
captured(GameMgr.Inst, server)
assert(#server.characters == 1 and #server.skins == 1 and #server.other_character_sort == 0)
assert(GameMgr.Inst.characterInfo ~= server)
assert(GameMgr.Inst.skin_map[400202] == 1)
assert(GameUtility.item_owned(200002) and GameUtility.item_owned(400202))
assert(GameUtility.item_owned(30740023) and not GameUtility.item_owned(305001))
assert(not M.owned(99999999), 'unknown IDs must not become usable')
local cb = 0
local function success(err, res) assert(not err and res.error.code == 0); cb = cb + 1 end
local captured_send = LobbyNetMgr.SendRequest
local original_account = GameMgr.Inst.account_data
captured_send('Lobby', 'changeMainCharacter', { character_id = 200002 }, success)
captured_send('Lobby', 'changeCharacterSkin', { character_id = 200002, skin = 400202 }, success)
assert(#SENT == 0 and cb == 2)
assert(GameMgr.Inst.characterInfo.main_character_id == 200002)
assert(GameMgr.Inst.account_data.avatar_id == 400202)
assert(original_account.avatar_id == 400101)
assert(server.characters[1].skin == 400101)
assert(PREFS_SAVED >= 2 and PLAYER_PREFS['MJLocalPreview.appearance.v1.123']:find('200002:400202', 1, true))
-- A delayed server notification must not change or persist the selected trial.
local saved = PLAYER_PREFS['MJLocalPreview.appearance.v1.123']
GameMgr.SetMainCharacterAndSkin(200001, 400101)
assert(M.choices.character_id == 200002 and M.skins[200002] == 400202)
assert(GameMgr.Inst.characterInfo.main_character_id == 200002 and GameMgr.Inst.account_data.avatar_id == 400202)
assert(PLAYER_PREFS['MJLocalPreview.appearance.v1.123'] == saved)
for _, method in ipairs({ 'updateCharacterSort', 'setHiddenCharacter', 'setRandomCharacter',
  'changeCharacterView', 'changeCommonView' }) do captured_send('Lobby', method, {}, success) end
assert(#SENT == 0)
captured_send('Lobby', 'changeCharacterView', { character_id = 200002, slot = 4, item_id = 30990015 }, success)
captured_send('Lobby', 'changeCommonView', { slot = 4, value = 30990015 }, success)
captured_send('Lobby', 'saveCommonViews', { save_index = 0, is_use = 1,
  views = { { slot = 4, item_id = 30990015 }, { slot = 5, item_id = 30990016 } } }, success)
captured_send('Lobby', 'useCommonView', { index = 0 }, success)
captured_send('Lobby', 'useTitle', { title = 600002 }, success)
captured_send('Lobby', 'setLoadingImage', { images = { 201201 } }, success)
assert(#SENT == 0 and GameMgr.Inst.loading_image[1] == 201201)
local other = { account_id = 999, avatar_id = 400101, character = { charid = 200001, skin = 400101 } }
local own = { account_id = 123, avatar_id = 400101, character = { charid = 200001, skin = 400101 },
  views = { { slot = 4, item_id = 1 } } }
local packet = { other, own }
local resources = M.preload_players(packet)
assert(#resources == 3 and resources[1] == other and resources[2] == own)
assert(resources[3].character.skin == 400202, 'trial assets must be loaded before room initialization')
local trial = M.players(packet, 123, GameUtility.EMJ_Mode.play)
assert(trial ~= packet and trial[1] == other and trial[2] ~= own)
assert(trial[2].character.charid == 200002 and trial[2].character.skin == 400202)
assert(trial[2].avatar_id == 400202 and trial[2].title == 600002)
assert(trial[2].character.views[1].item_id == 30990015)
assert(#trial[2].views == 2)
assert(own.character.skin == 400101 and own.views[1].item_id == 1)
-- Friend-room seats are sparse and stale packets cannot undo a local selection.
local room_packet = { [2] = own, [4] = other }
local room = M.room_players(room_packet)
assert(room[1] == nil and room[3] == nil and room[4] == other)
assert(room[2].avatar_id == 400202 and room_packet[2] == own)
local refreshed = 0
UI_FriendRoom = { Inst = { room_id = 12345, enable = true, players = room_packet,
  _refreshPlayerInfo = function(self, index)
    assert(index == 2 and self.players[2].avatar_id == M.skins[200002])
    refreshed = refreshed + 1
  end } }
captured_send('Lobby', 'changeCharacterSkin', { character_id = 200002, skin = 400201 }, success)
assert(refreshed == 1 and UI_FriendRoom.Inst.players[2].avatar_id == 400201 and own.avatar_id == 400101)
captured_send('Lobby', 'changeCharacterSkin', { character_id = 200002, skin = 400202 }, success)
assert(refreshed == 2)
local desktop = { active = true, player_datas = packet, myaccountid = 123, mode = 0 }
M.desktop(desktop)
assert(desktop.player_datas[2].character.skin == 400202 and desktop.myPlayerData == desktop.player_datas[2])
assert(packet[2].character.skin == 400101)
assert(M.players(packet, 0, GameUtility.EMJ_Mode.play) == packet, 'spectator must stay intact')
assert(M.players(packet, 123, GameUtility.EMJ_Mode.paipu) == packet, 'replay must stay intact')
-- Reconnect: run a fresh incoming character response and a fresh room packet.
captured(GameMgr.Inst, server)
assert(GameMgr.Inst.characterInfo.main_character_id == 200002)
assert(M.players(packet, 123, 0)[2].character.skin == 400202)
captured_send('Lobby', 'fetchInfo', {}, nil)
assert(#SENT == 1 and SENT[1].method == 'fetchInfo')
local titles
captured_send('Lobby', 'fetchTitleList', {}, function(_, res) titles = res.title_list end)
assert(#SENT == 2 and #titles == 2)
-- A module with the same method name must not be swallowed.
captured_send('FastTest', 'useTitle', {}, nil)
assert(#SENT == 3)
-- Losing the network guard disables ownership preview.
M.guard_ready = false
assert(not M.owned(200002))
assert(UI_Bag.GetCount(305044) == 0 and UI_Bag.GetCount(305001) == 7)
assert(#UI_Bag.FindItemByCategory(5, 4) == 0, 'disabled guard must restore real cosmetic list')
assert(M.players(packet, 123, 0) == packet)
M.guard_ready = true
-- Profiles remain separate, and switching back reloads the saved main and skins.
GameMgr.Inst.account_id = 456
M.account(GameMgr.Inst)
assert(next(M.choices) == nil and next(M.skins) == nil and next(M.groups) == nil)
GameMgr.Inst.account_id = 123
M.account(GameMgr.Inst)
assert(M.choices.character_id == 200002 and M.skins[200002] == 400202 and next(M.groups) == nil)
-- Outdated IDs and a skin belonging to another character cannot be restored.
PLAYER_PREFS['MJLocalPreview.appearance.v1.456'] = '1;999999;200001:400202,200002:999999'
GameMgr.Inst.account_id = 456
M.account(GameMgr.Inst)
assert(next(M.choices) == nil and next(M.skins) == nil)
PLAYER_PREFS['MJLocalPreview.appearance.v1.456'] = 'return os.execute("unexpected")'
M.account_id = nil; M.account(GameMgr.Inst)
assert(next(M.choices) == nil and next(M.skins) == nil)
