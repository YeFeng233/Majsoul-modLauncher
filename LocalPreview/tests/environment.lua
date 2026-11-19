LOGS, SENT = {}, {}
PLAYER_PREFS = PLAYER_PREFS or {}
UnityEngine = { Debug = { Log = function(s) LOGS[#LOGS + 1] = s end }, PlayerPrefs = {
  GetString = function(key, default) return PLAYER_PREFS[key] or default end,
  SetString = function(key, value) PLAYER_PREFS[key] = value end,
  Save = function() PREFS_SAVED = (PREFS_SAVED or 0) + 1 end,
} }
local cfg = {
  character = {
    [200001] = { init_skin = 400101 }, [200002] = { init_skin = 400201 },
  },
  skin = { [400101] = { character_id = 200001 }, [400102] = { character_id = 200001 },
    [400201] = { character_id = 200002 }, [400202] = { character_id = 200002 } },
  item = { [30740023] = { id = 30740023, category = 8, type = 1 }, [30990015] = { category = 4 },
    [305001] = { category = 1 }, [305044] = { id = 305044, category = 5, type = 4, sort = 1 } },
  title = { [600001] = {}, [600002] = {} },
}
ExcelMgr = { GetData = function(_, section, id) return (cfg[section] or {})[id] end }
GameMgr = { Inst = { account_id = 123, account_data = { avatar_id = 400101 },
  skin_map = {}, finished_endings = {}, rewarded_endings = {} } }
setmetatable(GameMgr.Inst, { __index = GameMgr })
GameUtility = { EIDType = { character = 2, skin = 4, item = 3, title = 6 }, EMJ_Mode = { play = 0, paipu = 1 } }
GameUtility.get_id_type = function(id)
  if id >= 30000000 then return 3 end
  return math.floor(id / 100000)
end
UI_Bag = { Inst = { getItem = { [305001] = { item_id = 305001, stack = 7, data = cfg.item[305001] } } }, GetCount = function() return 0 end }
UI_LiaosheSelect = { Inst = { Init = function()
  assert(#GameMgr.Inst.characterInfo.characters == 2, 'catalogue must be present before native UI rebuild')
end } }
Tools = {
  IsArrayContains = function(t, id) for _, v in ipairs(t) do if v == id then return true end end return false end,
  GetRealTime = function() return 1 end,
}
UI_ErrorInfo = { HandleInfo = function() error('unexpected game status report') end }
NetRouteGroup = { Inst = { SendRequest = function(_, module, method, data, cb)
  SENT[#SENT + 1] = { module = module, method = method, data = data }
  if cb then cb(nil, { error = { code = 0 }, title_list = { 600001 } }) end
end } }
NetRouteGroup_Entrance = {}
LobbyNetMgr = { requestCountMap = {}, requestTimeListMap = {} }
